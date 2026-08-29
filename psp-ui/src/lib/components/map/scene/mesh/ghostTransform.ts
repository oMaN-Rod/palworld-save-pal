import type { PlacementAnchor, Quat, Vec3 } from '$types';

export function yawQuat(yaw: number): Quat {
	const half = yaw / 2;
	return { x: 0, y: 0, z: Math.sin(half), w: Math.cos(half) };
}

function quatMul(a: Quat, b: Quat): Quat {
	return {
		x: a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
		y: a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
		z: a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
		w: a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z
	};
}

function rotateVec(q: Quat, v: Vec3): Vec3 {
	const tx = 2 * (q.y * v.z - q.z * v.y);
	const ty = 2 * (q.z * v.x - q.x * v.z);
	const tz = 2 * (q.x * v.y - q.y * v.x);
	return {
		x: v.x + q.w * tx + q.y * tz - q.z * ty,
		y: v.y + q.w * ty + q.z * tx - q.x * tz,
		z: v.z + q.w * tz + q.x * ty - q.y * tx
	};
}

export function composeWorld(
	anchor: PlacementAnchor,
	relative: { translation: Vec3; rotation: Quat; scale: Vec3 }
): { translation: Vec3; rotation: Quat; scale: Vec3 } {
	const aq = yawQuat(anchor.yaw);
	const offset = rotateVec(aq, relative.translation);
	return {
		translation: { x: anchor.x + offset.x, y: anchor.y + offset.y, z: anchor.z + offset.z },
		rotation: quatMul(aq, relative.rotation),
		scale: relative.scale
	};
}
