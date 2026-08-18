// Per-fragment shading for scenery instances: samples the map raster directly in
// the fragment shader (rather than one flat colour per instance) and shades with a
// wrap-ambient key light plus a slope-driven cliff blend.
import * as THREE from 'three';

export const SCENERY_AMBIENT = 0.95;
// How far a cliff face is pulled toward its own luminance and darkened beneath the
// ground it belongs to, so it reads as recessed rather than as a pale slab.
export const SCENERY_CLIFF_DESAT = 0.6;
export const SCENERY_CLIFF_DARKEN = 0.95;
// Matches MapLibre's raster-brightness-min remap (out = min + (1-min)*in) so scenery
// and the raster under it stay tonally identical; dark biomes otherwise crush to black.
export const SCENERY_LIFT = 0.14;
// Cool grey, not warm stone: this multiplies the sampled biome colour, so a warm
// value pushes every rock face toward khaki. Blue exceeds red to counteract that.
export const SCENERY_CLIFF_COLOR = 0xbcc6d6;
export const SCENERY_CLIFF_START = 0.55;
export const SCENERY_CLIFF_END = 0.9;
export const SCENERY_SUN = [0.5, 0.3, 1.0];

const VERTEX_SHADER = `
varying vec2 vMapUv;
varying vec3 vNormalW;
void main() {
	vec4 wp = modelMatrix * instanceMatrix * vec4(position, 1.0);
	vMapUv = wp.xy;
	vNormalW = normalize(mat3(modelMatrix) * mat3(instanceMatrix) * normal);
	gl_Position = projectionMatrix * viewMatrix * wp;
}
`;

const FRAGMENT_SHADER = `
uniform sampler2D uMap;
uniform float uHasMap;
uniform vec3 uBase;
uniform vec3 uSun;
uniform float uAmbient;
uniform vec3 uCliff;
uniform float uCliffStart;
uniform float uCliffEnd;
uniform float uCliffDesat;
uniform float uCliffDarken;
uniform float uLift;
uniform float uOpacity;
varying vec2 vMapUv;
varying vec3 vNormalW;
void main() {
	vec3 base = uHasMap > 0.5 ? texture2D(uMap, vMapUv).rgb : uBase;
	base = uLift + (1.0 - uLift) * base;
	vec3 n = normalize(vNormalW);
	float slope = 1.0 - clamp(abs(n.z), 0.0, 1.0);
	float cliff = smoothstep(uCliffStart, uCliffEnd, slope);
	// Rock is derived from the biome colour rather than laid over it: desaturate
	// toward luminance, tint, then darken. Purely multiplicative, so a face can
	// never come out brighter than the ground it belongs to.
	float lum = dot(base, vec3(0.2126, 0.7152, 0.0722));
	vec3 rock = mix(base, vec3(lum), uCliffDesat) * uCliff * uCliffDarken;
	base = mix(base, rock, cliff);
	float key = max(dot(n, normalize(uSun)), 0.0);
	vec3 lit = base * (uAmbient + (1.0 - uAmbient) * key);
	gl_FragColor = vec4(lit, uOpacity);
}
`;

export function createSceneryMaterial(): THREE.ShaderMaterial {
	return new THREE.ShaderMaterial({
		uniforms: {
			uMap: { value: null },
			uHasMap: { value: 0 },
			uBase: { value: new THREE.Color(0x8a8578) },
			uSun: { value: new THREE.Vector3(...SCENERY_SUN) },
			uAmbient: { value: SCENERY_AMBIENT },
			uCliff: { value: new THREE.Color(SCENERY_CLIFF_COLOR) },
			uCliffStart: { value: SCENERY_CLIFF_START },
			uCliffEnd: { value: SCENERY_CLIFF_END },
			uCliffDesat: { value: SCENERY_CLIFF_DESAT },
			uCliffDarken: { value: SCENERY_CLIFF_DARKEN },
			uLift: { value: SCENERY_LIFT },
			uOpacity: { value: 1 }
		},
		vertexShader: VERTEX_SHADER,
		fragmentShader: FRAGMENT_SHADER
	});
}

// Swaps the map texture in (or clears it) without rebuilding the material.
export function setSceneryMaterialMap(
	material: THREE.ShaderMaterial,
	texture: THREE.Texture | null
): void {
	material.uniforms.uMap.value = texture;
	material.uniforms.uHasMap.value = texture ? 1 : 0;
}

// depthWrite stays true so rock instances within the one InstancedMesh depth-test
// correctly against each other.
export function setSceneryMaterialOpacity(material: THREE.ShaderMaterial, opacity: number): void {
	material.uniforms.uOpacity.value = opacity;
	const blended = opacity < 1;
	if (material.transparent !== blended) {
		material.transparent = blended;
		material.needsUpdate = true;
	}
}
