// A MapLibre CustomLayerInterface that renders placed structures as instanced
// three.js meshes — real game meshes where the parts manifest resolves them,
// falling back to procedural proxy geometry otherwise. Placement/height reuse
// structurePlacement + the same verticalScale the DEM terrain uses. Technique:
// MapLibre "add a 3D model using three.js" example (shared GL context,
// MercatorCoordinate placement).
import * as THREE from 'three';
import { MercatorCoordinate, type CustomLayerInterface, type Map as MLMap } from 'maplibre-gl';
import type { BaseStructure, Footprint } from '$types';
import type { MapArea } from './utils';
import { buildArchetypeGeometry } from './proxyGeometry';
import { ueYawToThreeQuaternion } from './coords3d';
import { structurePlacement, structureAnchor } from './structurePlacement';
import { structureFillColor } from './styles';
import { DEFAULT_STRUCTURE_FOOTPRINT, lookupFootprint } from './features';
import { structureParts, requestMesh, meshFailed, onMeshLoaded, type ManifestPart } from './meshLibrary';
import { partLocalMatrix, type MeshPart } from './meshPlacement';
import { PickIndex } from './pickIndex';
import { decodePickBytes } from './pickEncoding';

// Both mesh and proxy geometry are authored Y-up; MapLibre's mercator world
// (fed through mainMatrix with an identity camera view) is Z-up. A naive
// rotationX = PI/2 base flip (the MapLibre three.js example's reconciliation)
// reflects the horizontal plane relative to flat mode's ground truth
// (buildStructureFC, features.ts): local +X would land on world +x and local
// +Z on world -y, a determinant -1 map, while flat mode's yaw rotation is a
// proper (determinant +1) rotation. MESH_FLIP is the determinant +1
// replacement -- local +X -> world -y, +Z -> world +x, +Y (up) unchanged --
// verified against flat mode by meshOrientation.test.ts and
// proxyOrientation.test.ts. Shared by both the mesh and proxy paths.
const MESH_FLIP = new THREE.Matrix4().makeBasis(
	new THREE.Vector3(0, -1, 0),
	new THREE.Vector3(0, 0, 1),
	new THREE.Vector3(1, 0, 0)
);

// Identity comes from gl_InstanceID plus a per-group base rather than a
// per-instance attribute: geometries are shared between buckets, so attaching an
// InstancedBufferAttribute would corrupt the sibling bucket or leak a fresh
// buffer on every update. The colour is computed per vertex and passed flat so
// interpolation cannot corrupt it, and a raw ShaderMaterial keeps three's colour
// management off the value.
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

export type Group = { mesh: THREE.InstancedMesh; keys: string[]; colorHex: string; pickBase: number };

type MeshItem = { s: BaseStructure; fp: Footprint; part: ManifestPart };
type MeshBucket = { colorHex: string; mesh: string; items: MeshItem[] };
type ProxyBucket = { fp: Footprint; colorHex: string; items: BaseStructure[] };

// Pure (no WebGL, no InstancedMesh) per-instance transforms, split out of
// update() so C1 (cm-vs-metre scale) and C2 (box-offset leaking into the mesh
// path) are each covered by a unit test instead of only being reachable
// through a live renderer.
export function meshInstanceMatrix(
	s: BaseStructure,
	part: MeshPart,
	area: MapArea,
	verticalScale: number,
	cmToMerc: number
): THREE.Matrix4 {
	// Real game meshes use the raw actor transform: unlike the proxy path there
	// is no origin/half-height centering to compensate for a synthetic box.
	const p = structureAnchor(s, area);
	const anchor = MercatorCoordinate.fromLngLat([p.lng, p.lat], p.altitudeCm * verticalScale);
	const yawRotation = new THREE.Matrix4().makeRotationFromQuaternion(ueYawToThreeQuaternion(p.yaw));
	const rotation = MESH_FLIP.clone().multiply(yawRotation);
	const scale = new THREE.Matrix4().makeScale(cmToMerc, cmToMerc, cmToMerc);
	return new THREE.Matrix4()
		.makeTranslation(anchor.x, anchor.y, anchor.z)
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
	const anchor = MercatorCoordinate.fromLngLat([p.lng, p.lat], originCm * verticalScale);
	const yawRotation = new THREE.Matrix4().makeRotationFromQuaternion(ueYawToThreeQuaternion(p.yaw));
	const rotation = MESH_FLIP.clone().multiply(yawRotation);
	const scale = new THREE.Matrix4().makeScale(cmToMerc, cmToMerc, cmToMerc);
	return new THREE.Matrix4()
		.makeTranslation(anchor.x, anchor.y, anchor.z)
		.multiply(rotation)
		.multiply(scale);
}

// Pure CSS-pixel -> device-pixel conversion for the GPU colour-pick read, split
// out of runPick() for the same reason meshInstanceMatrix/proxyInstanceMatrix
// above are: it's exercised without a live WebGL context. WebGL's framebuffer
// origin is bottom-left; the canvas/CSS origin is top-left, hence the Y flip.
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

// Shared across layer instances (Map.svelte tears down and recreates this
// CustomLayerInterface on every detailed-mode toggle) because the renderer
// wraps MapLibre's own GL context/canvas, not one this layer owns. Disposing
// it on toggle-off would tear down three's WebGLAttributes -- and with it the
// GPU buffers backing every mesh/proxy geometry cached in meshLibrary and
// proxyGeometry -- while those caches deliberately outlive the layer so a
// toggle-on reuses them instead of re-uploading. Keeping one renderer alive
// for the map's lifetime avoids leaking a buffer set on every toggle.
// Module-scoped so each id is reported once, not once per pan/zoom rebuild.
const reportedProxy = new Set<string>();

let sharedRenderer: THREE.WebGLRenderer | null = null;
let sharedContext: WebGLRenderingContext | WebGL2RenderingContext | null = null;

function getSharedRenderer(
	canvas: HTMLCanvasElement,
	gl: WebGLRenderingContext | WebGL2RenderingContext
): THREE.WebGLRenderer {
	if (sharedRenderer && sharedContext === gl) return sharedRenderer;
	sharedRenderer = new THREE.WebGLRenderer({ canvas, context: gl, antialias: true });
	sharedRenderer.autoClear = false;
	sharedContext = gl;
	return sharedRenderer;
}

export type StructureLayer = CustomLayerInterface & {
	update(
		structures: BaseStructure[],
		footprints: Record<string, Footprint>,
		area: MapArea,
		verticalScale: number
	): void;
	setHover(key: string | null): void;
	requestPick(x: number, y: number, cb: (key: string | null) => void): void;
	dispose(): void;
	// Test-only introspection: onAdd's renderer setup needs a live GL context, but
	// update()'s bucket/group building does not, so tests attach a stub map
	// directly rather than going through onAdd.
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
			// Geometry is never disposed here: mesh-library geometry (requestMesh) and
			// proxy geometry (buildArchetypeGeometry) both live in module-level caches
			// shared across updates and layer instances. Disposing a cached geometry
			// frees its GPU buffers while the object stays cached, so a later cache
			// hit would render nothing. Only the per-update material and the
			// InstancedMesh's own instance-attribute buffers belong to this layer.
			(g.mesh.material as THREE.Material).dispose();
			g.mesh.dispose();
		}
		groups.length = 0;
		pickIndex.reset();
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

		// Taken from the live canvas, not renderer.getDrawingBufferSize(): that getter
		// returns width * pixelRatio captured at construction time and is only kept
		// current by setSize/setDrawingBufferSize, neither of which this layer calls
		// (the shared renderer wraps MapLibre's own canvas). After any canvas resize
		// it would go stale while `ratio` below -- already read from the live canvas
		// -- stays correct, displacing every pick.
		const canvas = renderer.domElement;
		const width = canvas.width;
		const height = canvas.height;
		// Derived from the canvas rather than renderer.getPixelRatio(): the shared
		// renderer wraps MapLibre's own canvas/context and is never told its ratio
		// (setPixelRatio is never called), so that getter is always 1 regardless of
		// the canvas's actual device-pixel size.
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

		update(structures, footprints, area, verticalScale) {
			lastArgs = [structures, footprints, area, verticalScale];
			if (!map || disposed) return;
			clearGroups();
			const center = map.getCenter();
			const merc = MercatorCoordinate.fromLngLat([center.lng, center.lat], 0);
			const mPerUnit = merc.meterInMercatorCoordinateUnits();
			// cm -> mercator units: cm * verticalScale (metres) * mPerUnit.
			const cmToMerc = verticalScale * mPerUnit;

			const meshBuckets = new Map<string, MeshBucket>();
			const proxyBuckets = new Map<string, ProxyBucket>();

			function addProxy(s: BaseStructure, fp: Footprint, colorHex: string, why: string) {
				if (!reportedProxy.has(s.map_object_id)) {
					reportedProxy.add(s.map_object_id);
					console.info(`[structure3d] proxy fallback: ${s.map_object_id} (${why})`);
				}
				const archetype = fp.archetype ?? 'box';
				const key = `proxy:${archetype}|${fp.sx}|${fp.sy}|${fp.sz}|${colorHex}`;
				let b = proxyBuckets.get(key);
				if (!b) {
					b = { fp, colorHex, items: [] };
					proxyBuckets.set(key, b);
				}
				b.items.push(s);
			}

			for (const s of structures) {
				const fp = lookupFootprint(footprints, s.map_object_id) ?? DEFAULT_STRUCTURE_FOOTPRINT;
				const colorHex = structureFillColor(fp.typeA, fp.material);
				const parts = structureParts(s.map_object_id);

				if (parts && parts.length > 0) {
					// Resolve every part before committing to a render path: a mesh part
					// with no size data can't stand in for the whole structure, so a
					// single permanently-failed part falls the whole structure back to
					// its proxy box rather than mixing partial mesh + a misplaced proxy.
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
						addProxy(s, fp, colorHex, `mesh failed: ${failedMesh}`);
						continue;
					}
					if (anyLoading) continue; // still loading; onMeshLoaded triggers a rebuild
					for (const part of resolvedParts) {
						const key = `mesh:${part.mesh}|${colorHex}`;
						let b = meshBuckets.get(key);
						if (!b) {
							b = { colorHex, mesh: part.mesh, items: [] };
							meshBuckets.set(key, b);
						}
						b.items.push({ s, fp, part });
					}
					continue;
				}
				addProxy(s, fp, colorHex, parts ? 'manifest entry has no parts' : 'no manifest entry');
			}

			// The mercator transform mirrors handedness relative to three's convention,
			// which flips winding and makes FrontSide backface-cull the visible faces.
			function addInstancedGroup(
				geom: THREE.BufferGeometry,
				count: number
			): { inst: THREE.InstancedMesh; keys: string[] } {
				const material = new THREE.MeshLambertMaterial({ color: 0xffffff, side: THREE.DoubleSide });
				const inst = new THREE.InstancedMesh(geom, material, count);
				inst.instanceColor = new THREE.InstancedBufferAttribute(new Float32Array(count * 3), 3);
				inst.frustumCulled = false;
				return { inst, keys: [] };
			}

			function finalizeGroup(inst: THREE.InstancedMesh, keys: string[], colorHex: string) {
				const pickBase = pickIndex.add(keys);
				inst.instanceMatrix.needsUpdate = true;
				if (inst.instanceColor) inst.instanceColor.needsUpdate = true;
				scene.add(inst);
				groups.push({ mesh: inst, keys, colorHex, pickBase });
				inst.onBeforeRender = (_renderer, _scene, _camera, _geometry, material) => {
					const sm = material as THREE.ShaderMaterial;
					// scene.overrideMaterial substitutes one shared ShaderMaterial instance for
					// every render item, so three only re-uploads its uniforms when the program
					// or material identity changes -- never between instances sharing this
					// material. uniformsNeedUpdate forces the upload for every group's draw call.
					if (!sm.uniforms?.uPickBase) return;
					sm.uniforms.uPickBase.value = pickBase;
					sm.uniformsNeedUpdate = true;
				};
			}

			for (const b of meshBuckets.values()) {
				const geom = requestMesh(b.mesh);
				if (!geom) continue;
				const { inst, keys } = addInstancedGroup(geom, b.items.length);
				b.items.forEach(({ s, part }, i) => {
					inst.setMatrixAt(i, meshInstanceMatrix(s, part, area, verticalScale, cmToMerc));
					color.set(b.colorHex);
					inst.setColorAt(i, color);
					keys.push(s.instance_id);
				});
				finalizeGroup(inst, keys, b.colorHex);
			}

			for (const b of proxyBuckets.values()) {
				const archetype = b.fp.archetype ?? 'box';
				const geom = buildArchetypeGeometry(archetype, b.fp.sx, b.fp.sy, b.fp.sz);
				const { inst, keys } = addInstancedGroup(geom, b.items.length);
				b.items.forEach((s, i) => {
					inst.setMatrixAt(i, proxyInstanceMatrix(s, b.fp, archetype, area, verticalScale, cmToMerc));
					color.set(b.colorHex);
					inst.setColorAt(i, color);
					keys.push(s.instance_id);
				});
				finalizeGroup(inst, keys, b.colorHex);
			}

			applyHover();
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
			// The pick pass shares MapLibre's context, so it runs here rather than from a
			// standalone call: issuing draws and a readPixels outside MapLibre's own render
			// loop risks leaving the context in a state it does not expect.
			if (pendingPick) {
				runPick();
				renderer.resetState();
			}
			// MapLibre's shared depth buffer isn't cleared for us; without this, three's
			// meshes inherit stale depth state and don't occlude each other correctly.
			renderer.clearDepth();
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
			pendingPick = null;
			pickTarget?.dispose();
			pickTarget = null;
			pickMaterial.dispose();
			clearGroups();
			// renderer is the module-level shared renderer (see getSharedRenderer) --
			// it is intentionally not disposed here, only released by this instance.
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

	// A settle (success or permanent failure) can fire synchronously out of
	// requestMesh() while update() is still iterating buckets; deferring the
	// rebuild to a microtask guarantees it never re-enters a running update(),
	// and the rebuildQueued flag coalesces a burst of settles (many meshes
	// finishing close together) into a single rebuild.
	const unsubscribeMeshLoaded = onMeshLoaded(() => {
		if (rebuildQueued) return;
		rebuildQueued = true;
		queueMicrotask(() => {
			rebuildQueued = false;
			if (lastArgs) layer.update(...lastArgs);
		});
	});

	return layer;
}
