// A blueprint part's transform inside its actor, in UE centimetres, expressed in
// three-space via the same axis swap coords3d uses: three.(x, y, z) = ue.(x, z, y).
import * as THREE from 'three';

const DEG = Math.PI / 180;

export type MeshPart = {
	loc: [number, number, number];
	rot: [number, number, number];
	scale: [number, number, number];
};

export function ueEulerToThreeQuaternion(
	pitch: number,
	yaw: number,
	roll: number
): THREE.Quaternion {
	// UE yaw is about Z (up), which is three's Y after the swap; the handedness flip negates it.
	return new THREE.Quaternion().setFromEuler(
		new THREE.Euler(roll * DEG, -yaw * DEG, pitch * DEG, 'YZX')
	);
}

export function partLocalMatrix(part: MeshPart): THREE.Matrix4 {
	const position = new THREE.Vector3(part.loc[0], part.loc[2], part.loc[1]);
	const quaternion = ueEulerToThreeQuaternion(part.rot[0], part.rot[1], part.rot[2]);
	const scale = new THREE.Vector3(part.scale[0], part.scale[2], part.scale[1]);
	return new THREE.Matrix4().compose(position, quaternion, scale);
}
