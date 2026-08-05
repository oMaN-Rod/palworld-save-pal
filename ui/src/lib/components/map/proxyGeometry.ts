// Parametric proxy geometry per archetype, in cm, anchored so a mesh's base sits
// at local y=0 (foundations top-anchored). Geometry
// is cached by archetype+size so many identical pieces share one buffer.
import * as THREE from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';

const cache = new Map<string, THREE.BufferGeometry>();

function bottomBox(sx: number, sz: number, sy: number, y0 = 0): THREE.BufferGeometry {
	const g = new THREE.BoxGeometry(sx, sz, sy);
	g.translate(0, y0 + sz / 2, 0);
	return g;
}

function build(archetype: string, sx: number, sy: number, sz: number): THREE.BufferGeometry {
	switch (archetype) {
		case 'foundation': {
			const g = new THREE.BoxGeometry(sx, sz, sy);
			g.translate(0, -sz / 2, 0);
			return g;
		}
		case 'pyramidRoof': {
			const g = new THREE.ConeGeometry((Math.max(sx, sy) / 2) * Math.SQRT2 * 0.72, sz, 4);
			g.rotateY(Math.PI / 4);
			g.translate(0, sz / 2, 0);
			return g;
		}
		case 'gableRoof': {
			const hx = sx / 2;
			const hz = sy / 2;
			const verts = new Float32Array([
				-hx, 0, -hz, hx, 0, -hz, 0, sz, -hz,
				-hx, 0, hz, hx, 0, hz, 0, sz, hz
			]);
			const idx = [0, 1, 2, 3, 5, 4, 0, 2, 5, 0, 5, 3, 1, 4, 5, 1, 5, 2, 0, 3, 4, 0, 4, 1];
			const g = new THREE.BufferGeometry();
			g.setAttribute('position', new THREE.BufferAttribute(verts, 3));
			g.setIndex(idx);
			g.computeVertexNormals();
			return g;
		}
		case 'stair': {
			const steps = 4;
			const parts: THREE.BufferGeometry[] = [];
			for (let i = 0; i < steps; i++) {
				const depth = sy * (1 - i / steps);
				const g = new THREE.BoxGeometry(sx, sz / steps, depth);
				g.translate(0, (sz / steps) * (i + 0.5), (sy - depth) / 2);
				parts.push(g);
			}
			return mergeGeometries(parts);
		}
		case 'fence': {
			const t = Math.min(sx, sy) * 0.12 + 4;
			const post = (x: number) => {
				const g = new THREE.BoxGeometry(t, sz, t);
				g.translate(x, sz / 2, 0);
				return g;
			};
			const rail = (y: number) => {
				const g = new THREE.BoxGeometry(sx, t, t);
				g.translate(0, y, 0);
				return g;
			};
			return mergeGeometries([post(-sx / 2 + t / 2), post(sx / 2 - t / 2), rail(sz * 0.35), rail(sz * 0.75)]);
		}
		case 'chest': {
			const base = new THREE.BoxGeometry(sx, sz * 0.7, sy);
			base.translate(0, sz * 0.35, 0);
			const lid = new THREE.BoxGeometry(sx, sz * 0.3, sy);
			lid.translate(0, sz * 0.7 + (sz * 0.3) / 2, 0);
			return mergeGeometries([base, lid]);
		}
		case 'workstation': {
			const top = new THREE.BoxGeometry(sx, sz * 0.15, sy);
			top.translate(0, sz * 0.85 + (sz * 0.15) / 2, 0);
			const legX = sx * 0.08;
			const legZ = sy * 0.08;
			const legH = sz * 0.85;
			const leg = (x: number, z: number) => {
				const g = new THREE.BoxGeometry(legX, legH, legZ);
				g.translate(x, legH / 2, z);
				return g;
			};
			const cx = sx / 2 - legX / 2;
			const cz = sy / 2 - legZ / 2;
			return mergeGeometries([top, leg(-cx, -cz), leg(cx, -cz), leg(-cx, cz), leg(cx, cz)]);
		}
		case 'lampPost': {
			const post = new THREE.CylinderGeometry(6, 6, sz, 12);
			post.translate(0, sz / 2, 0);
			const head = new THREE.SphereGeometry(Math.min(sx, sy) * 0.3);
			head.translate(0, sz, 0);
			return mergeGeometries([post, head]);
		}
		case 'torch': {
			const postH = sz * 0.8;
			const post = new THREE.CylinderGeometry(4, 4, postH, 8);
			post.translate(0, postH / 2, 0);
			const flameH = sz * 0.2;
			const flame = new THREE.ConeGeometry(Math.min(sx, sy) * 0.25, flameH, 6);
			flame.translate(0, postH + flameH / 2, 0);
			return mergeGeometries([post, flame]);
		}
		case 'turret': {
			const baseH = sz * 0.5;
			const base = new THREE.CylinderGeometry(sx * 0.4, sx * 0.5, baseH, 8);
			base.translate(0, baseH / 2, 0);
			const barrel = new THREE.BoxGeometry(sx * 0.15, sz * 0.2, sy * 0.9);
			barrel.translate(0, sz * 0.6 + (sz * 0.2) / 2, 0);
			return mergeGeometries([base, barrel]);
		}
		case 'tank': {
			const r = Math.min(sx, sy) * 0.5;
			const g = new THREE.CylinderGeometry(r, r, sz, 12);
			g.translate(0, sz / 2, 0);
			return g;
		}
		case 'chimneyStack': {
			const body = new THREE.BoxGeometry(sx, sz * 0.7, sy);
			body.translate(0, sz * 0.35, 0);
			const stackX = sx * 0.3;
			const stackZ = sy * 0.3;
			const stack = new THREE.BoxGeometry(stackX, sz * 0.5, stackZ);
			stack.translate(sx / 2 - stackX / 2, sz * 0.7 + (sz * 0.5) / 2, sy / 2 - stackZ / 2);
			return mergeGeometries([body, stack]);
		}
		case 'ladder': {
			const railX = sx * 0.1;
			const railZ = sy * 0.1;
			const rail = (x: number) => {
				const g = new THREE.BoxGeometry(railX, sz, railZ);
				g.translate(x, sz / 2, 0);
				return g;
			};
			const rungH = sz * 0.06;
			const rung = (y: number) => {
				const g = new THREE.BoxGeometry(sx, rungH, sy * 0.06);
				g.translate(0, y, 0);
				return g;
			};
			const parts = [rail(-sx / 2 + railX / 2), rail(sx / 2 - railX / 2)];
			for (let i = 1; i <= 4; i++) parts.push(rung((sz * i) / 5));
			return mergeGeometries(parts);
		}
		case 'wallDoor':
		case 'wallWindow':
		case 'wallGate': {
			const holeW = archetype === 'wallGate' ? sx * 0.7 : sx * 0.4;
			const jambW = (sx - holeW) / 2;
			const parts: THREE.BufferGeometry[] = [];
			const left = new THREE.BoxGeometry(jambW, sz, sy);
			left.translate(-(holeW / 2 + jambW / 2), sz / 2, 0);
			const right = new THREE.BoxGeometry(jambW, sz, sy);
			right.translate(holeW / 2 + jambW / 2, sz / 2, 0);
			parts.push(left, right);
			if (archetype === 'wallDoor' || archetype === 'wallGate') {
				const lintelH = sz * 0.2;
				const lintel = new THREE.BoxGeometry(holeW, lintelH, sy);
				lintel.translate(0, sz - lintelH / 2, 0);
				parts.push(lintel);
			} else {
				const sillH = sz * 0.35;
				const sill = new THREE.BoxGeometry(holeW, sillH, sy);
				sill.translate(0, sillH / 2, 0);
				const head = new THREE.BoxGeometry(holeW, sillH, sy);
				head.translate(0, sz - sillH / 2, 0);
				parts.push(sill, head);
			}
			return mergeGeometries(parts);
		}
		default:
			return bottomBox(sx, sz, sy);
	}
}

export function buildArchetypeGeometry(
	archetype: string,
	sx: number,
	sy: number,
	sz: number
): THREE.BufferGeometry {
	const key = `${archetype}|${sx}|${sy}|${sz}`;
	const hit = cache.get(key);
	if (hit) return hit;
	const g = build(archetype, sx, sy, sz);
	cache.set(key, g);
	return g;
}
