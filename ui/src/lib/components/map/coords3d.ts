// UE (cm, Z-up, left-handed) <-> Three (m, Y-up, right-handed):
// three.(x,y,z) = ue.(x,z,y), a single axis swap turning a
// left-handed frame right-handed while keeping up as up. Our structure bake
// keeps yaw only (pitch/roll discarded in guild.rs), so we expose the yaw case.
import * as THREE from 'three';

const UP_Y = new THREE.Vector3(0, 1, 0);

export function ueYawToThreeQuaternion(yaw: number): THREE.Quaternion {
	// UE yaw is a rotation about world Z (up). After the x<->z-with-y swap, up is
	// Three's +Y; the handedness flip negates the angle.
	return new THREE.Quaternion().setFromAxisAngle(UP_Y, -yaw);
}
