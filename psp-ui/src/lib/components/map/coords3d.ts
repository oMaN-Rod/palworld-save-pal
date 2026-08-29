// UE (cm, Z-up, left-handed) <-> Three (m, Y-up, right-handed):
// three.(x,y,z) = ue.(x,z,y), one axis swap that turns a left-handed frame
// right-handed while keeping up as up.
import * as THREE from 'three';

const UP_Y = new THREE.Vector3(0, 1, 0);

export function ueYawToThreeQuaternion(yaw: number): THREE.Quaternion {
	// UE yaw rotates about world Z. After the swap up is Three's +Y, and the
	// handedness flip negates the angle.
	return new THREE.Quaternion().setFromAxisAngle(UP_Y, -yaw);
}

// The position swap is a linear map P exchanging the y/z rows (det P = -1, a
// reflection). A rotation carries across by conjugation R' = P R P^-1, and P is
// its own inverse, which works out to (qx, qz, qy, -qw). Note the w sign flips
// too: this is not just the position swap. That flip cancels only in the
// yaw-only case (q and -q are the same rotation), which is why yaw alone looked
// correct while pitch and roll did not.
export function ueQuatToThree(qx: number, qy: number, qz: number, qw: number): THREE.Quaternion {
	return new THREE.Quaternion(qx, qz, qy, -qw);
}
