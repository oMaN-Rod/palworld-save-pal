import * as THREE from 'three';
import { MercatorCoordinate, type ExpressionSpecification } from 'maplibre-gl';
import type { Spawn } from '$types';
import { worldToPixel, type MapArea } from './utils';
import { pixelToLngLat } from './mercator';
import { PORTAL_HEIGHT_CM, PORTAL_TAPER_RATIO, CORE_COLOR } from './palPortal';

export type FastTravelState = 'unknown' | 'locked' | 'unlocked';
export type RelicState = 'unknown' | 'uncollected' | 'collected';
// Reuses the save's own spawn_type union so a new kind fails PORTAL_COLORS.palRing
// indexing at compile time rather than resolving to nothing at runtime. Bounty is
// excluded rather than coloured: those spawns name no pal, so they never reach a
// pal model and a ring colour for them would be unreachable.
export type PalRingKind = Exclude<Spawn['spawn_type'], 'bounty'>;

// The radius each marker type's ring and beam both derive from, in centimetres
// before the size slider. Alpha, boss and predator use palPortal's
// PORTAL_RADIUS_CM instead.
export const RELIC_RADIUS_CM = 140;
export const FAST_TRAVEL_RADIUS_CM = 220;

const AMBER_COLOR = new THREE.Color(0xffa726);
const COLLECTED_COLOR = new THREE.Color(0x66bb6a);
const PREDATOR_COLOR = new THREE.Color(0xff3b30);

export const PORTAL_COLORS = {
	fastTravel: {
		unknown: CORE_COLOR,
		locked: AMBER_COLOR,
		unlocked: CORE_COLOR
	},
	relic: {
		unknown: CORE_COLOR,
		uncollected: AMBER_COLOR,
		collected: COLLECTED_COLOR
	},
	palRing: {
		alpha: CORE_COLOR,
		boss: CORE_COLOR,
		predator: PREDATOR_COLOR
	}
} as const satisfies Record<string, Record<string, THREE.Color>>;

export function fastTravelPortalColor(state: FastTravelState): THREE.Color {
	return PORTAL_COLORS.fastTravel[state];
}

export function relicPortalColor(state: RelicState): THREE.Color {
	return PORTAL_COLORS.relic[state];
}

export function palRingColor(kind: PalRingKind): THREE.Color {
	return PORTAL_COLORS.palRing[kind];
}

type HexPalette<T> = { [K in keyof T]: T[K] extends THREE.Color ? string : HexPalette<T[K]> };

function toHexPalette<T extends Record<string, Record<string, THREE.Color>>>(
	colors: T
): HexPalette<T> {
	const result = {} as HexPalette<T>;
	for (const kind of Object.keys(colors) as (keyof T)[]) {
		const hexStates: Record<string, string> = {};
		for (const [state, color] of Object.entries(colors[kind])) {
			hexStates[state] = `#${color.getHexString()}`;
		}
		result[kind] = hexStates as HexPalette<T>[typeof kind];
	}
	return result;
}

/**
 * `PORTAL_COLORS` as CSS hex strings, for consumers that can't use a THREE.Color
 * -- the ground rings drawn as maplibre paint expressions. Derived rather than
 * duplicated so a beam colour change can't drift the two apart.
 */
export const PORTAL_HEX = toHexPalette(PORTAL_COLORS);

/**
 * A maplibre `match` over `["get", "state"]` covering every state of the palette,
 * built from `PORTAL_HEX` so it can never omit one the beam table knows about.
 * `palRing` has no `unknown` state, so its caller must supply a fallback.
 */
export function portalRingColorExpression(
	kind: keyof typeof PORTAL_HEX,
	fallback?: string
): ExpressionSpecification {
	const palette: Record<string, string> = PORTAL_HEX[kind];
	const arms = Object.entries(palette).flatMap(([state, hex]) => [state, hex]);
	return ['match', ['get', 'state'], ...arms, fallback ?? palette.unknown] as unknown as ExpressionSpecification;
}

export function mapObjectPortalMatrix(
	worldX: number,
	worldY: number,
	worldZ: number,
	area: MapArea,
	cmToMerc: number,
	scale: number
): THREE.Matrix4 {
	const [px, py] = worldToPixel(worldX, worldY, area);
	const [lng, lat] = pixelToLngLat(px, py);
	const anchor = MercatorCoordinate.fromLngLat([lng, lat]);
	const mercScale = scale * cmToMerc;
	const anchorZ = worldZ * cmToMerc;
	return new THREE.Matrix4()
		.makeTranslation(anchor.x, anchor.y, anchorZ)
		.multiply(new THREE.Matrix4().makeScale(mercScale, mercScale, mercScale));
}

const VERTEX = /* glsl */ `
attribute vec3 aColor;
varying vec3 vColor;
varying vec3 vNormalW;
varying vec3 vViewW;
varying float vHeight;
void main() {
	vColor = aColor;
	vHeight = uv.y;
	vec4 world = instanceMatrix * vec4(position, 1.0);
	vNormalW = normalize(mat3(instanceMatrix) * normal);
	vViewW = normalize(cameraPosition - world.xyz);
	gl_Position = projectionMatrix * modelViewMatrix * world;
}
`;

const FRAGMENT = /* glsl */ `
varying vec3 vColor;
varying vec3 vNormalW;
varying vec3 vViewW;
varying float vHeight;
void main() {
	// Same rim-brightening trick as the boss portal beam: the surface reads
	// brighter where it turns away from the camera, so the tube's edges glow
	// and slide around it as the camera orbits, driven only by camera moves the
	// map already repaints for.
	float rim = pow(1.0 - abs(dot(normalize(vNormalW), normalize(vViewW))), 2.0);
	float fade = pow(1.0 - clamp(vHeight, 0.0, 1.0), 1.5);
	float a = rim * fade;
	vec3 c = mix(vColor, vec3(1.0), rim * 0.5);
	gl_FragColor = vec4(c * a, a);
}
`;

function portalMaterial(): THREE.ShaderMaterial {
	return new THREE.ShaderMaterial({
		vertexShader: VERTEX,
		fragmentShader: FRAGMENT,
		transparent: true,
		blending: THREE.AdditiveBlending,
		depthTest: true,
		depthWrite: false,
		side: THREE.DoubleSide
	});
}

export function createMapObjectPortalMesh(count: number, radiusCm: number): THREE.InstancedMesh {
	// Built per call, not shared at module level: aColor is attached to the
	// geometry, so a shared one would have its buffer overwritten by whichever
	// layer built last. radiusCm is the same value the caller's ring polygon uses,
	// making the two concentric by construction rather than by tuning.
	const geometry = new THREE.CylinderGeometry(
		radiusCm * PORTAL_TAPER_RATIO,
		radiusCm,
		PORTAL_HEIGHT_CM,
		36,
		1,
		true
	);
	// CylinderGeometry is built around +Y with its origin at mid-height; up is +z
	// here, so stand it upright and lift its base onto the ground anchor.
	geometry.rotateX(Math.PI / 2);
	geometry.translate(0, 0, PORTAL_HEIGHT_CM / 2);

	geometry.setAttribute(
		'aColor',
		new THREE.InstancedBufferAttribute(new Float32Array(count * 3), 3)
	);

	const mesh = new THREE.InstancedMesh(geometry, portalMaterial(), count);
	mesh.frustumCulled = false;
	return mesh;
}

export function disposeMapObjectPortalMesh(mesh: THREE.InstancedMesh): void {
	mesh.geometry.dispose();
	(mesh.material as THREE.ShaderMaterial).dispose();
	mesh.dispose();
}
