// A MapLibre CustomLayerInterface that renders placed structures as instanced
// three.js proxy meshes. Placement/height reuse structurePlacement + the same
// verticalScale the DEM terrain uses. Technique: MapLibre "add a 3D model using
// three.js" example (shared GL context, MercatorCoordinate placement).
import * as THREE from 'three';
import { MercatorCoordinate, type CustomLayerInterface, type Map as MLMap } from 'maplibre-gl';
import type { BaseStructure, Footprint } from '$types';
import type { MapArea } from './utils';
import { buildArchetypeGeometry } from './proxyGeometry';
import { ueYawToThreeQuaternion } from './coords3d';
import { structurePlacement } from './structurePlacement';
import { structureFillColor } from './styles';
import { DEFAULT_STRUCTURE_FOOTPRINT } from './features';

// Proxy geometry is authored Y-up; MapLibre's mercator world (fed through
// mainMatrix with an identity camera view) is Z-up. This base rotation stands
// meshes upright — the same reconciliation the MapLibre three.js example does
// with rotationX = PI/2.
const UP_FLIP = new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(1, 0, 0), Math.PI / 2);

type Group = { mesh: THREE.InstancedMesh; keys: string[]; colorHex: string };

export type StructureLayer = CustomLayerInterface & {
	update(
		structures: BaseStructure[],
		footprints: Record<string, Footprint>,
		area: MapArea,
		verticalScale: number
	): void;
	setHover(key: string | null): void;
	dispose(): void;
};

export function createStructureLayer(opts: { id: string }): StructureLayer {
	const scene = new THREE.Scene();
	const camera = new THREE.Camera();
	let renderer: THREE.WebGLRenderer | null = null;
	let map: MLMap | null = null;
	const groups: Group[] = [];
	let hoverKey: string | null = null;
	const dummy = new THREE.Object3D();
	const color = new THREE.Color();

	scene.add(new THREE.AmbientLight(0xffffff, 0.7));
	const dir = new THREE.DirectionalLight(0xffffff, 0.9);
	dir.position.set(0.5, 1, 0.3);
	scene.add(dir);

	function clearGroups() {
		for (const g of groups) {
			scene.remove(g.mesh);
			g.mesh.geometry.dispose();
			(g.mesh.material as THREE.Material).dispose();
			g.mesh.dispose();
		}
		groups.length = 0;
	}

	const layer: StructureLayer = {
		id: opts.id,
		type: 'custom',
		renderingMode: '3d',

		onAdd(m, gl) {
			map = m;
			renderer = new THREE.WebGLRenderer({
				canvas: m.getCanvas(),
				context: gl as WebGLRenderingContext,
				antialias: true
			});
			renderer.autoClear = false;
		},

		update(structures, footprints, area, verticalScale) {
			if (!map) return;
			clearGroups();
			const center = map.getCenter();
			const merc = MercatorCoordinate.fromLngLat([center.lng, center.lat], 0);
			const mPerUnit = merc.meterInMercatorCoordinateUnits();
			// cm -> mercator units: cm * verticalScale (metres) * mPerUnit.
			const cmToMerc = verticalScale * mPerUnit;

			const buckets = new Map<
				string,
				{ fp: Footprint; items: { s: BaseStructure }[]; colorHex: string }
			>();
			for (const s of structures) {
				const fp = footprints[s.map_object_id] ?? DEFAULT_STRUCTURE_FOOTPRINT;
				const archetype = fp.archetype ?? 'box';
				const colorHex = structureFillColor(fp.typeA, fp.material);
				const key = `${archetype}|${fp.sx}|${fp.sy}|${fp.sz}|${colorHex}`;
				let b = buckets.get(key);
				if (!b) {
					b = { fp, items: [], colorHex };
					buckets.set(key, b);
				}
				b.items.push({ s });
			}

			for (const b of buckets.values()) {
				const archetype = b.fp.archetype ?? 'box';
				const geom = buildArchetypeGeometry(archetype, b.fp.sx, b.fp.sy, b.fp.sz);
				// The mercator transform mirrors handedness relative to three's convention,
				// which flips winding and makes FrontSide backface-cull the visible faces.
				const material = new THREE.MeshLambertMaterial({ color: 0xffffff, side: THREE.DoubleSide });
				const inst = new THREE.InstancedMesh(geom, material, b.items.length);
				inst.instanceColor = new THREE.InstancedBufferAttribute(
					new Float32Array(b.items.length * 3),
					3
				);
				const keys: string[] = [];
				b.items.forEach(({ s }, i) => {
					const p = structurePlacement(s, b.fp, area, verticalScale);
					const halfH = p.footprintCm.sz / 2;
					const originCm = p.altitudeCm + (archetype === 'foundation' ? halfH : -halfH);
					const anchor = MercatorCoordinate.fromLngLat([p.lng, p.lat], originCm * verticalScale);
					dummy.position.set(anchor.x, anchor.y, anchor.z);
					dummy.quaternion.copy(UP_FLIP).multiply(ueYawToThreeQuaternion(p.yaw));
					dummy.scale.setScalar(cmToMerc);
					dummy.updateMatrix();
					inst.setMatrixAt(i, dummy.matrix);
					color.set(b.colorHex);
					inst.setColorAt(i, color);
					keys.push(s.instance_id);
				});
				inst.instanceMatrix.needsUpdate = true;
				if (inst.instanceColor) inst.instanceColor.needsUpdate = true;
				inst.frustumCulled = false;
				scene.add(inst);
				groups.push({ mesh: inst, keys, colorHex: b.colorHex });
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
			// MapLibre's shared depth buffer isn't cleared for us; without this, three's
			// meshes inherit stale depth state and don't occlude each other correctly.
			renderer.clearDepth();
			renderer.render(scene, camera);
		},

		dispose() {
			clearGroups();
			renderer?.dispose();
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

	return layer;
}
