// Pal sizes differ by more than an order of magnitude, so the camera is placed from each model's own bounding sphere.
import * as THREE from 'three';

export const FIT_MARGIN = 1.12;

// The narrower of the two half-angles governs: a tall, narrow panel runs out of horizontal room first even though the model fits vertically.
export function fitDistance(radius: number, fovDeg: number, aspect: number): number {
	const vertical = ((fovDeg / 2) * Math.PI) / 180;
	const horizontal = Math.atan(Math.tan(vertical) * aspect);
	const half = Math.min(vertical, horizontal);
	// Floor of 1 so a zero-radius model doesn't put the camera at the origin, clipping through everything.
	return Math.max((radius * FIT_MARGIN) / Math.sin(half), 1);
}

// Radius measured from vertices, not Box3 (whose half-diagonal overstates by up to sqrt(3) on a rounded
// Pal). Turning about the vertical axis leaves each vertex's distance from the centre unchanged, so this
// radius holds at every angle of the turntable.
export function palBounds(root: THREE.Object3D): { centre: THREE.Vector3; radius: number } {
	root.updateMatrixWorld(true);
	const box = new THREE.Box3().setFromObject(root);
	if (box.isEmpty()) return { centre: new THREE.Vector3(), radius: 1 };

	const centre = box.getCenter(new THREE.Vector3());
	const v = new THREE.Vector3();
	let farthestSq = 0;
	root.traverse((child) => {
		const mesh = child as THREE.Mesh;
		if (!mesh.isMesh || !mesh.geometry) return;
		const position = mesh.geometry.getAttribute('position');
		if (!position) return;
		for (let i = 0; i < position.count; i++) {
			v.fromBufferAttribute(position, i).applyMatrix4(mesh.matrixWorld).sub(centre);
			farthestSq = Math.max(farthestSq, v.lengthSq());
		}
	});

	return { centre, radius: Math.max(Math.sqrt(farthestSq), 1) };
}
