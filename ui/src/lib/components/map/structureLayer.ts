// Technique: MapLibre's "add a 3D model using three.js" example (shared GL
// context, MercatorCoordinate placement).
import * as THREE from 'three';
import { MercatorCoordinate, type CustomLayerInterface, type Map as MLMap } from 'maplibre-gl';
import type { BaseStructure, Footprint } from '$types';
import type { MapArea } from './utils';
import { buildArchetypeGeometry } from './proxyGeometry';
import { ueYawToThreeQuaternion } from './coords3d';
import { structurePlacement, structureAnchor } from './structurePlacement';
import { structureFillColor } from './styles';
import { materialOpacities } from './mapColors.svelte';
import { DEFAULT_STRUCTURE_FOOTPRINT, lookupFootprint } from './features';
import {
	structureParts,
	requestMesh,
	meshFailed,
	onMeshLoaded,
	requestTexturedMesh,
	onTexturedMeshLoaded,
	STRUCTURE_MODEL_DIR,
	type ManifestPart,
	type TexturedMeshBundle
} from './meshLibrary';
import { partLocalMatrix, type MeshPart } from './meshPlacement';
import { PickIndex } from './pickIndex';
import { decodePickBytes } from './pickEncoding';
import { bakeStructureInstance, composeStructureMatrix, STRUCTURE_BAKE_STRIDE } from './structureInstances';

// Both mesh and proxy geometry are authored Y-up; MapLibre's mercator world is Z-up.
// The naive rotationX = PI/2 flip is a determinant -1 map and reflects the plane
// relative to flat mode's ground truth; MESH_FLIP is the determinant +1 replacement
// (local +X -> world -y, +Z -> world +x, +Y unchanged), verified by
// meshOrientation.test.ts and proxyOrientation.test.ts.
export const MESH_FLIP = new THREE.Matrix4().makeBasis(
	new THREE.Vector3(0, -1, 0),
	new THREE.Vector3(0, 0, 1),
	new THREE.Vector3(1, 0, 0)
);

// Identity comes from gl_InstanceID plus a per-group base rather than a per-instance
// attribute: geometries are shared between buckets, so an InstancedBufferAttribute
// here would corrupt the sibling bucket. `flat` keeps interpolation from corrupting it.
const PICK_VERT = `
uniform float uPickBase;
flat out vec3 vPick;
void main() {
	float id = uPickBase + float(gl_InstanceID) + 1.0;
	float r = floor(id / 65536.0);
	float g = floor(mod(id, 65536.0) / 256.0);
	float b = mod(id, 256.0);
	vPick = vec3(r, g, b) / 255.0;
	gl_Position = projectionMatrix * modelViewMatrix * instanceMatrix * vec4(position, 1.0);
}
`;

// Under glslVersion GLSL3, three does not alias gl_FragColor to a declared out
// variable (that shim is only emitted for GLSL1 output) -- see WebGLProgram.js's
// pc_fragColor handling -- so the output must be declared explicitly here.
const PICK_FRAG = `
flat in vec3 vPick;
layout(location = 0) out vec4 pc_fragColor;
void main() {
	pc_fragColor = vec4(vPick, 1.0);
}
`;

export type Group = {
	mesh: THREE.InstancedMesh;
	keys: string[];
	colorHex: string;
	pickBase: number;
	// Camera-independent per-instance data, so setVerticalScale can recompose every
	// matrix without touching groups, geometry or materials.
	baked: Float32Array;
	// An extra transform composeStructureMatrix does not fold in; null on the proxy
	// path, which has no such offset.
	partMatrices: THREE.Matrix4[] | null;
};

type MeshItem = { s: BaseStructure; fp: Footprint; part: ManifestPart };
type MeshBucket = { colorHex: string; opacity: number; mesh: string; items: MeshItem[] };
type ProxyBucket = { fp: Footprint; colorHex: string; opacity: number; items: BaseStructure[] };

// Pure (no WebGL, no InstancedMesh) per-instance transform, split out of update()
// so it is unit-testable without a live renderer.
export function meshInstanceMatrix(
	s: BaseStructure,
	part: MeshPart,
	area: MapArea,
	_verticalScale: number,
	cmToMerc: number
): THREE.Matrix4 {
	// Real game meshes use the raw actor transform: unlike the proxy path there
	// is no origin/half-height centering to compensate for a synthetic box.
	const p = structureAnchor(s, area);
	// Z goes through cmToMerc directly, not fromLngLat's altitude argument, which would
	// divide by this instance's own latitude rather than the camera centre's.
	const anchor = MercatorCoordinate.fromLngLat([p.lng, p.lat]);
	const anchorZ = p.altitudeCm * cmToMerc;
	const yawRotation = new THREE.Matrix4().makeRotationFromQuaternion(ueYawToThreeQuaternion(p.yaw));
	const rotation = MESH_FLIP.clone().multiply(yawRotation);
	const scale = new THREE.Matrix4().makeScale(cmToMerc, cmToMerc, cmToMerc);
	return new THREE.Matrix4()
		.makeTranslation(anchor.x, anchor.y, anchorZ)
		.multiply(rotation)
		.multiply(scale)
		.multiply(partLocalMatrix(part));
}

export function proxyInstanceMatrix(
	s: BaseStructure,
	fp: Footprint,
	archetype: string,
	area: MapArea,
	verticalScale: number,
	cmToMerc: number
): THREE.Matrix4 {
	const p = structurePlacement(s, fp, area, verticalScale);
	const halfH = p.footprintCm.sz / 2;
	const originCm = p.altitudeCm + (archetype === 'foundation' ? halfH : -halfH);
	// See meshInstanceMatrix above: Z goes through cmToMerc directly, not
	// fromLngLat's per-instance-latitude altitude argument.
	const anchor = MercatorCoordinate.fromLngLat([p.lng, p.lat]);
	const anchorZ = originCm * cmToMerc;
	const yawRotation = new THREE.Matrix4().makeRotationFromQuaternion(ueYawToThreeQuaternion(p.yaw));
	const rotation = MESH_FLIP.clone().multiply(yawRotation);
	const scale = new THREE.Matrix4().makeScale(cmToMerc, cmToMerc, cmToMerc);
	return new THREE.Matrix4()
		.makeTranslation(anchor.x, anchor.y, anchorZ)
		.multiply(rotation)
		.multiply(scale);
}

// WebGL's framebuffer origin is bottom-left; the canvas/CSS origin is top-left,
// hence the Y flip.
export function pickPixelCoords(
	cssX: number,
	cssY: number,
	ratio: number,
	width: number,
	height: number
): { x: number; y: number } | null {
	const x = Math.round(cssX * ratio);
	const y = Math.round(height - 1 - cssY * ratio);
	if (x < 0 || y < 0 || x >= width || y >= height) return null;
	return { x, y };
}

const reportedProxy = new Set<string>();

// Shared across layer instances because the renderer wraps MapLibre's own GL
// context/canvas, not one this layer owns. Disposing it on toggle-off would tear
// down three's WebGLAttributes, and with them the GPU buffers backing every
// mesh/proxy geometry cached in meshLibrary/proxyGeometry, which outlive the layer.
let sharedRenderer: THREE.WebGLRenderer | null = null;
let sharedContext: WebGLRenderingContext | WebGL2RenderingContext | null = null;

export function getSharedRenderer(
	canvas: HTMLCanvasElement,
	gl: WebGLRenderingContext | WebGL2RenderingContext
): THREE.WebGLRenderer {
	if (sharedRenderer && sharedContext === gl) return sharedRenderer;
	sharedRenderer = new THREE.WebGLRenderer({ canvas, context: gl, antialias: true });
	sharedRenderer.autoClear = false;
	sharedContext = gl;
	return sharedRenderer;
}

export function peekSharedRenderer(): THREE.WebGLRenderer | null {
	return sharedRenderer;
}

// Cloned so mutating opacity here cannot bleed back into the shared cache.
export function texturedGroupMaterial(
	bundle: TexturedMeshBundle,
	opacity: number
): THREE.Material | THREE.Material[] {
	const withOpacity = (material: THREE.Material): THREE.Material => {
		const clone = material.clone();
		clone.transparent = opacity < 1;
		clone.opacity = opacity;
		return clone;
	};
	return Array.isArray(bundle.material)
		? bundle.material.map(withOpacity)
		: withOpacity(bundle.material);
}

export type StructureLayer = CustomLayerInterface & {
	update(
		structures: BaseStructure[],
		footprints: Record<string, Footprint>,
		area: MapArea,
		verticalScale: number,
		textured?: boolean
	): void;
	setVerticalScale(verticalScale: number): void;
	setHover(key: string | null): void;
	requestPick(x: number, y: number, cb: (key: string | null) => void): void;
	dispose(): void;
	// Test-only: onAdd needs a live GL context, but update()'s bucket/group building
	// does not, so tests attach a stub map directly rather than going through onAdd.
	attachMapForTest(map: MLMap): void;
	groupsForTest(): Group[];
	keyAtForTest(index: number): string | null;
};

export function createStructureLayer(opts: { id: string }): StructureLayer {
	const scene = new THREE.Scene();
	const camera = new THREE.Camera();
	let renderer: THREE.WebGLRenderer | null = null;
	let map: MLMap | null = null;
	const groups: Group[] = [];
	const pickIndex = new PickIndex();
	let hoverKey: string | null = null;
	const color = new THREE.Color();

	let lastArgs: Parameters<StructureLayer['update']> | null = null;
	let rebuildQueued = false;
	let disposed = false;
	let isWebGL2 = false;
	let pickTarget: THREE.WebGLRenderTarget | null = null;
	let pendingPick: { x: number; y: number; cb: (key: string | null) => void } | null = null;
	const pickBuffer = new Uint8Array(4);
	const pickMaterial = new THREE.ShaderMaterial({
		glslVersion: THREE.GLSL3,
		uniforms: { uPickBase: { value: 0 } },
		vertexShader: PICK_VERT,
		fragmentShader: PICK_FRAG,
		side: THREE.DoubleSide
	});

	scene.add(new THREE.AmbientLight(0xffffff, 0.7));
	const dir = new THREE.DirectionalLight(0xffffff, 0.9);
	dir.position.set(0.5, 1, 0.3);
	scene.add(dir);

	function clearGroups() {
		for (const g of groups) {
			scene.remove(g.mesh);
			// Geometry is never disposed here: it lives in module-level caches (meshLibrary,
			// proxyGeometry) shared across updates and layer instances, so freeing its GPU
			// buffers here would leave a later cache hit rendering nothing. Only the
			// per-update material (including a textured group's cloned one) belongs to this layer.
			const material = g.mesh.material;
			if (Array.isArray(material)) {
				for (const m of material) m.dispose();
			} else {
				material.dispose();
			}
			g.mesh.dispose();
		}
		groups.length = 0;
		pickIndex.reset();
	}

	// Shared by update() and setVerticalScale() so the two never drift. partMatrix
	// does not depend on cmToMerc, so it post-multiplies rather than folding into the bake.
	const scratch = new THREE.Matrix4();
	function applyBakedMatrix(
		inst: THREE.InstancedMesh,
		index: number,
		baked: Float32Array,
		cmToMerc: number,
		partMatrix: THREE.Matrix4 | null
	) {
		composeStructureMatrix(baked, index * STRUCTURE_BAKE_STRIDE, cmToMerc, scratch);
		if (partMatrix) scratch.multiply(partMatrix);
		inst.setMatrixAt(index, scratch);
	}

	function ensurePickTarget(width: number, height: number): THREE.WebGLRenderTarget {
		if (pickTarget && pickTarget.width === width && pickTarget.height === height) return pickTarget;
		pickTarget?.dispose();
		pickTarget = new THREE.WebGLRenderTarget(width, height, {
			type: THREE.UnsignedByteType,
			colorSpace: THREE.NoColorSpace,
			depthBuffer: true
		});
		return pickTarget;
	}

	function runPick() {
		const request = pendingPick;
		pendingPick = null;
		if (!renderer || !request) return;

		// Taken from the live canvas, not renderer.getDrawingBufferSize(): that getter is
		// only kept current by setSize/setDrawingBufferSize, neither of which this layer
		// calls, and would go stale after a canvas resize, displacing every pick.
		const canvas = renderer.domElement;
		const width = canvas.width;
		const height = canvas.height;
		// Derived from the canvas rather than renderer.getPixelRatio(): setPixelRatio is
		// never called on this shared renderer, so that getter is always 1.
		const ratio = canvas.clientWidth > 0 ? canvas.width / canvas.clientWidth : 1;
		const coords = pickPixelCoords(request.x, request.y, ratio, width, height);
		if (!coords) {
			request.cb(null);
			return;
		}
		const { x: px, y: py } = coords;

		const target = ensurePickTarget(width, height);
		scene.overrideMaterial = pickMaterial;
		try {
			renderer.setRenderTarget(target);
			renderer.setScissorTest(true);
			renderer.setScissor(px, py, 1, 1);
			renderer.setClearColor(0x000000, 1);
			renderer.clear(true, true, false);
			renderer.render(scene, camera);
			renderer.readRenderTargetPixels(target, px, py, 1, 1, pickBuffer);
		} finally {
			renderer.setScissorTest(false);
			renderer.setRenderTarget(null);
			scene.overrideMaterial = null;
		}

		const index = decodePickBytes(pickBuffer[0], pickBuffer[1], pickBuffer[2]);
		request.cb(index < 0 ? null : pickIndex.keyAt(index));
	}

	const layer: StructureLayer = {
		id: opts.id,
		type: 'custom',
		renderingMode: '3d',

		onAdd(m, gl) {
			map = m;
			isWebGL2 = typeof WebGL2RenderingContext !== 'undefined' && gl instanceof WebGL2RenderingContext;
			renderer = getSharedRenderer(m.getCanvas(), gl as WebGLRenderingContext);
		},

		update(structures, footprints, area, verticalScale, textured = false) {
			lastArgs = [structures, footprints, area, verticalScale, textured];
			if (!map || disposed) return;
			clearGroups();
			const center = map.getCenter();
			const merc = MercatorCoordinate.fromLngLat([center.lng, center.lat], 0);
			const mPerUnit = merc.meterInMercatorCoordinateUnits();
			// cm -> mercator units: cm * verticalScale (metres) * mPerUnit.
			const cmToMerc = verticalScale * mPerUnit;

			const meshBuckets = new Map<string, MeshBucket>();
			const proxyBuckets = new Map<string, ProxyBucket>();
			const opacities = materialOpacities();

			function addProxy(s: BaseStructure, fp: Footprint, colorHex: string, opacity: number, why: string) {
				if (!reportedProxy.has(s.map_object_id)) {
					reportedProxy.add(s.map_object_id);
					console.info(`[structure3d] proxy fallback: ${s.map_object_id} (${why})`);
				}
				const archetype = fp.archetype ?? 'box';
				const key = `proxy:${archetype}|${fp.sx}|${fp.sy}|${fp.sz}|${colorHex}|${opacity.toFixed(2)}`;
				let b = proxyBuckets.get(key);
				if (!b) {
					b = { fp, colorHex, opacity, items: [] };
					proxyBuckets.set(key, b);
				}
				b.items.push(s);
			}

			for (const s of structures) {
				const fp = lookupFootprint(footprints, s.map_object_id) ?? DEFAULT_STRUCTURE_FOOTPRINT;
				const colorHex = structureFillColor(fp.typeA, fp.material);
				const opacity = opacities[fp.material ?? ''] ?? 1;
				const parts = structureParts(s.map_object_id);

				if (parts && parts.length > 0) {
					// A single permanently-failed part falls the whole structure back to its
					// proxy box, rather than mixing a partial mesh with a misplaced proxy.
					const resolvedParts: ManifestPart[] = [];
					let failedMesh: string | null = null;
					let anyLoading = false;
					for (const part of parts) {
						const geom = requestMesh(part.mesh);
						if (geom) {
							resolvedParts.push(part);
						} else if (meshFailed(part.mesh)) {
							failedMesh = part.mesh;
						} else {
							anyLoading = true;
						}
					}
					if (failedMesh) {
						addProxy(s, fp, colorHex, opacity, `mesh failed: ${failedMesh}`);
						continue;
					}
					if (anyLoading) continue; // still loading; onMeshLoaded triggers a rebuild
					for (const part of resolvedParts) {
						const key = `mesh:${part.mesh}|${colorHex}|${opacity.toFixed(2)}`;
						let b = meshBuckets.get(key);
						if (!b) {
							b = { colorHex, opacity, mesh: part.mesh, items: [] };
							meshBuckets.set(key, b);
						}
						b.items.push({ s, fp, part });
					}
					continue;
				}
				addProxy(s, fp, colorHex, opacity, parts ? 'manifest entry has no parts' : 'no manifest entry');
			}

			// The mercator transform mirrors handedness relative to three's convention,
			// which flips winding and makes FrontSide backface-cull the visible faces.
			function addInstancedGroup(
				geom: THREE.BufferGeometry,
				count: number,
				opacity: number
			): { inst: THREE.InstancedMesh; keys: string[] } {
				// Transparent groups must still write depth: this layer shares MapLibre's depth
				// buffer, and pixels left at the cleared far value get repainted by later passes,
				// making the glass disappear entirely (at the cost of glass occluding glass).
				const material = new THREE.MeshLambertMaterial({
					color: 0xffffff,
					side: THREE.DoubleSide,
					transparent: opacity < 1,
					opacity
				});
				const inst = new THREE.InstancedMesh(geom, material, count);
				inst.instanceColor = new THREE.InstancedBufferAttribute(new Float32Array(count * 3), 3);
				inst.frustumCulled = false;
				// The pick pass swaps in scene.overrideMaterial and so ignores opacity: an
				// opacity-0 group would stay invisible yet keep intercepting clicks.
				inst.visible = opacity > 0;
				return { inst, keys: [] };
			}

			// With no instanceColor here, instances render the glb's own texture until
			// applyHover() lazily creates one, initialised to white.
			function addTexturedInstancedGroup(
				bundle: TexturedMeshBundle,
				count: number,
				opacity: number
			): { inst: THREE.InstancedMesh; keys: string[] } {
				const material = texturedGroupMaterial(bundle, opacity);
				const inst = new THREE.InstancedMesh(bundle.geometry, material, count);
				inst.frustumCulled = false;
				inst.visible = opacity > 0;
				return { inst, keys: [] };
			}

			function finalizeGroup(
				inst: THREE.InstancedMesh,
				keys: string[],
				colorHex: string,
				baked: Float32Array,
				partMatrices: THREE.Matrix4[] | null
			) {
				const pickBase = pickIndex.add(keys);
				inst.instanceMatrix.needsUpdate = true;
				if (inst.instanceColor) inst.instanceColor.needsUpdate = true;
				scene.add(inst);
				groups.push({ mesh: inst, keys, colorHex, pickBase, baked, partMatrices });
				inst.onBeforeRender = (_renderer, _scene, _camera, _geometry, material) => {
					const sm = material as THREE.ShaderMaterial;
					// scene.overrideMaterial substitutes one shared ShaderMaterial for every render
					// item, so three only re-uploads uniforms on identity change; uniformsNeedUpdate
					// forces the upload for each group's draw call.
					if (!sm.uniforms?.uPickBase) return;
					sm.uniforms.uPickBase.value = pickBase;
					sm.uniformsNeedUpdate = true;
				};
			}

			for (const b of meshBuckets.values()) {
				const geom = requestMesh(b.mesh);
				if (!geom) continue;
				const baked = new Float32Array(b.items.length * STRUCTURE_BAKE_STRIDE);
				const partMatrices: THREE.Matrix4[] = [];
				b.items.forEach(({ s, part }, i) => {
					baked.set(bakeStructureInstance(structureAnchor(s, area)), i * STRUCTURE_BAKE_STRIDE);
					partMatrices.push(partLocalMatrix(part));
				});
				// While the textured cache is still resolving, this bucket renders flat-coloured;
				// onTexturedMeshLoaded requeues a rebuild once it lands.
				if (textured) {
					const bundle = requestTexturedMesh(b.mesh);
					if (bundle) {
						const { inst, keys } = addTexturedInstancedGroup(bundle, b.items.length, b.opacity);
						b.items.forEach(({ s }, i) => {
							applyBakedMatrix(inst, i, baked, cmToMerc, partMatrices[i]);
							keys.push(s.instance_id);
						});
						finalizeGroup(inst, keys, '#ffffff', baked, partMatrices);
						continue;
					}
				}
				const { inst, keys } = addInstancedGroup(geom, b.items.length, b.opacity);
				b.items.forEach(({ s }, i) => {
					applyBakedMatrix(inst, i, baked, cmToMerc, partMatrices[i]);
					color.set(b.colorHex);
					inst.setColorAt(i, color);
					keys.push(s.instance_id);
				});
				finalizeGroup(inst, keys, b.colorHex, baked, partMatrices);
			}

			for (const b of proxyBuckets.values()) {
				const archetype = b.fp.archetype ?? 'box';
				const geom = buildArchetypeGeometry(archetype, b.fp.sx, b.fp.sy, b.fp.sz);
				const { inst, keys } = addInstancedGroup(geom, b.items.length, b.opacity);
				const baked = new Float32Array(b.items.length * STRUCTURE_BAKE_STRIDE);
				b.items.forEach((s, i) => {
					const p = structurePlacement(s, b.fp, area, verticalScale);
					const halfH = p.footprintCm.sz / 2;
					const originCm = p.altitudeCm + (archetype === 'foundation' ? halfH : -halfH);
					baked.set(
						bakeStructureInstance({ lng: p.lng, lat: p.lat, altitudeCm: originCm, yaw: p.yaw }),
						i * STRUCTURE_BAKE_STRIDE
					);
					applyBakedMatrix(inst, i, baked, cmToMerc, null);
					color.set(b.colorHex);
					inst.setColorAt(i, color);
					keys.push(s.instance_id);
				});
				finalizeGroup(inst, keys, b.colorHex, baked, null);
			}

			applyHover();
			map.triggerRepaint();
		},

		// Camera-only counterpart to update(): recomposes matrices from baked data
		// without rebuilding groups, geometry or materials.
		setVerticalScale(verticalScale) {
			if (!map || disposed) return;
			const center = map.getCenter();
			const merc = MercatorCoordinate.fromLngLat([center.lng, center.lat], 0);
			const mPerUnit = merc.meterInMercatorCoordinateUnits();
			const cmToMerc = verticalScale * mPerUnit;
			for (const g of groups) {
				for (let i = 0; i < g.keys.length; i++) {
					applyBakedMatrix(g.mesh, i, g.baked, cmToMerc, g.partMatrices ? g.partMatrices[i] : null);
				}
				g.mesh.instanceMatrix.needsUpdate = true;
			}
			map.triggerRepaint();
		},

		setHover(key) {
			hoverKey = key;
			applyHover();
			map?.triggerRepaint();
		},

		render(_gl, args) {
			if (!renderer) return;
			const m = new THREE.Matrix4().fromArray(args.defaultProjectionData.mainMatrix);
			camera.projectionMatrix = m;
			renderer.resetState();
			// Runs here, sharing MapLibre's context, rather than from a standalone call:
			// issuing draws/readPixels outside MapLibre's own render loop risks leaving the
			// context in a state it does not expect.
			if (pendingPick) {
				runPick();
				renderer.resetState();
			}
			renderer.render(scene, camera);
		},

		requestPick(x, y, cb) {
			if (!isWebGL2 || disposed) {
				cb(null);
				return;
			}
			pendingPick = { x, y, cb };
			map?.triggerRepaint();
		},

		dispose() {
			disposed = true;
			unsubscribeMeshLoaded();
			unsubscribeTexturedMeshLoaded();
			pendingPick = null;
			pickTarget?.dispose();
			pickTarget = null;
			pickMaterial.dispose();
			clearGroups();
			// renderer is the module-level shared renderer; not disposed here, only released.
			renderer = null;
		},

		attachMapForTest(m) {
			map = m;
		},

		groupsForTest() {
			return groups;
		},

		keyAtForTest(index) {
			return pickIndex.keyAt(index);
		}
	};

	function applyHover() {
		for (const g of groups) {
			for (let i = 0; i < g.keys.length; i++) {
				color.set(g.keys[i] === hoverKey ? '#00e5ff' : g.colorHex);
				g.mesh.setColorAt(i, color);
			}
			if (g.mesh.instanceColor) g.mesh.instanceColor.needsUpdate = true;
		}
	}

	// A settle can fire synchronously out of requestMesh()/requestTexturedMesh() while
	// update() is still iterating buckets; the microtask keeps it from re-entering a
	// running update(), and rebuildQueued coalesces a burst of settles into one.
	function scheduleRebuild() {
		if (rebuildQueued) return;
		rebuildQueued = true;
		queueMicrotask(() => {
			rebuildQueued = false;
			if (lastArgs) layer.update(...lastArgs);
		});
	}

	// Scoped to the structure mesh directory: the cache is shared with the scenery
	// layer, whose hundreds of meshes would otherwise each trigger a full rebuild.
	const unsubscribeMeshLoaded = onMeshLoaded(scheduleRebuild, STRUCTURE_MODEL_DIR);
	// The textured cache settles independently of the plain one above, so a structure
	// already resolved for colour mode may still need this to rebuild once its texture lands.
	const unsubscribeTexturedMeshLoaded = onTexturedMeshLoaded(scheduleRebuild, STRUCTURE_MODEL_DIR);

	return layer;
}
