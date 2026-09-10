import * as THREE from 'three';
import { MercatorCoordinate } from 'maplibre-gl';
import { worldToPixel, type MapArea } from '../../geo/utils';
import { pixelToLngLat } from '../../geo/mercator';

export const PORTAL_RADIUS_CM = 180;
export const PORTAL_HEIGHT_CM = 1380;
// Purely cosmetic taper. The base radius is what must match the ground ring, so
// every beam geometry derives its top radius from this ratio rather than from
// its own absolute number.
export const PORTAL_TAPER_RATIO = 0.72 / 0.92;
export const PORTAL_DIM_DEFEATED = 0.35;
// Non-zero re-enables a time-driven rotation, which needs a triggerRepaint every
// frame. The default effect is view-driven and costs nothing while the camera
// is still.
export const PORTAL_SPIN = 0;

export const CORE_COLOR = new THREE.Color(0x4fc3ff);
const RIM_COLOR = new THREE.Color(0xa5e8ff);

export function portalIntensity(defeated: boolean): number {
	return defeated ? PORTAL_DIM_DEFEATED : 1;
}

export function portalInstanceMatrix(
	worldX: number,
	worldY: number,
	worldZ: number,
	area: MapArea,
	cmToMerc: number,
	palScale: number
): THREE.Matrix4 {
	const [px, py] = worldToPixel(worldX, worldY, area);
	const [lng, lat] = pixelToLngLat(px, py);
	const anchor = MercatorCoordinate.fromLngLat([lng, lat]);
	const scale = palScale * cmToMerc;
	const anchorZ = worldZ * cmToMerc;
	return new THREE.Matrix4()
		.makeTranslation(anchor.x, anchor.y, anchorZ)
		.multiply(new THREE.Matrix4().makeScale(scale, scale, scale));
}

const VERTEX = /* glsl */ `
attribute float aIntensity;
varying float vIntensity;
varying vec3 vNormalW;
varying vec3 vViewW;
varying float vHeight;
varying vec2 vLocal;
void main() {
	vIntensity = aIntensity;
	vHeight = uv.y;
	vLocal = position.xy;
	vec4 world = instanceMatrix * vec4(position, 1.0);
	vNormalW = normalize(mat3(instanceMatrix) * normal);
	vViewW = normalize(cameraPosition - world.xyz);
	gl_Position = projectionMatrix * modelViewMatrix * world;
}
`;

const COLUMN_FRAGMENT = /* glsl */ `
uniform vec3 uCore;
uniform vec3 uRim;
varying float vIntensity;
varying vec3 vNormalW;
varying vec3 vViewW;
varying float vHeight;
void main() {
	// Brightness rises where the surface turns away from the camera, so the
	// tube's edges read brighter than its centre and slide around it as the
	// camera orbits. This is what makes a cylinder look like light rather than
	// a solid, and it updates on camera moves the map already repaints for.
	float rim = pow(1.0 - abs(dot(normalize(vNormalW), normalize(vViewW))), 2.0);
	float fade = pow(1.0 - clamp(vHeight, 0.0, 1.0), 1.5);
	float a = rim * fade;
	vec3 c = mix(uCore, uRim, rim);
	gl_FragColor = vec4(c * a * vIntensity, a * vIntensity);
}
`;

function portalMaterial(fragmentShader: string): THREE.ShaderMaterial {
	return new THREE.ShaderMaterial({
		vertexShader: VERTEX,
		fragmentShader,
		uniforms: {
			uCore: { value: CORE_COLOR },
			uRim: { value: RIM_COLOR },
			uRadius: { value: PORTAL_RADIUS_CM }
		},
		transparent: true,
		blending: THREE.AdditiveBlending,
		depthTest: true,
		depthWrite: false,
		side: THREE.DoubleSide
	});
}

export function createPortalMeshes(count: number): {
	column: THREE.InstancedMesh;
} {
	// Built per call, not shared at module level: aIntensity is attached to the
	// geometry, so a shared one would have its buffer overwritten by whichever
	// layer built last.
	const columnGeo = new THREE.CylinderGeometry(
		PORTAL_RADIUS_CM * PORTAL_TAPER_RATIO,
		PORTAL_RADIUS_CM,
		PORTAL_HEIGHT_CM,
		36,
		1,
		true
	);
	// CylinderGeometry is built around +Y with its origin at mid-height; up is +z
	// here, so stand it upright and lift its base onto the ground anchor.
	columnGeo.rotateX(Math.PI / 2);
	columnGeo.translate(0, 0, PORTAL_HEIGHT_CM / 2);

	columnGeo.setAttribute(
		'aIntensity',
		new THREE.InstancedBufferAttribute(new Float32Array(count), 1)
	);

	const column = new THREE.InstancedMesh(columnGeo, portalMaterial(COLUMN_FRAGMENT), count);
	column.frustumCulled = false;
	return { column };
}

export function disposePortalMeshes(meshes: { column: THREE.InstancedMesh }): void {
	meshes.column.geometry.dispose();
	(meshes.column.material as THREE.ShaderMaterial).dispose();
	meshes.column.dispose();
}
