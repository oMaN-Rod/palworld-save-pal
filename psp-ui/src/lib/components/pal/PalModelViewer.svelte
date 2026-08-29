<script lang="ts">
	import * as THREE from 'three';
	import type { Snippet } from 'svelte';
	import {
		onPalMeshLoaded,
		palMeshFailed,
		palModelUrl,
		requestPalMesh
	} from '$components/map/palMeshLibrary';
	import { fitDistance, palBounds } from './palViewer';
	import { PalSpin } from './palSpin';
	import * as m from '$i18n/messages';

	let {
		characterKey,
		fallback
	}: {
		characterKey: string;
		fallback?: Snippet;
	} = $props();

	const FOV = 35;
	// Slightly down rather than level, so it reads as a display stand instead of a mugshot.
	const ELEVATION = (8 * Math.PI) / 180;

	const spin = new PalSpin();

	let status = $state<'loading' | 'ready' | 'unavailable'>('loading');
	// $state.raw: three.js objects are deeply structured, and $state's proxying breaks three's own identity checks.
	let stage = $state.raw<THREE.Group | null>(null);
	let source = $state.raw<THREE.Object3D | null>(null);
	let camera: THREE.PerspectiveCamera | null = null;
	let radius = 1;

	const modelUrl = $derived(palModelUrl(characterKey));

	function place() {
		if (!camera) return;
		const distance = fitDistance(radius, FOV, camera.aspect);
		camera.position.set(0, Math.sin(ELEVATION) * distance, Math.cos(ELEVATION) * distance);
		camera.near = Math.max(distance - radius * 2, distance / 100);
		camera.far = distance + radius * 4;
		camera.lookAt(0, 0, 0);
		camera.updateProjectionMatrix();
	}

	function mount(canvas: HTMLCanvasElement) {
		const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true });
		renderer.setClearAlpha(0);
		renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));

		const scene = new THREE.Scene();
		const group = new THREE.Group();
		scene.add(group);

		scene.add(new THREE.AmbientLight(0xffffff, 1.8));
		// Unlike palLayer, this scene keeps three's default +y up, so HemisphereLight needs no position correction.
		scene.add(new THREE.HemisphereLight(0xffffff, 0x8899aa, 1.1));
		const key = new THREE.DirectionalLight(0xffffff, 1.6);
		key.position.set(0.5, 0.9, 1);
		scene.add(key);
		const fill = new THREE.DirectionalLight(0xffffff, 0.5);
		fill.position.set(-0.8, 0.2, -0.6);
		scene.add(fill);

		camera = new THREE.PerspectiveCamera(FOV, 1, 1, 1000);

		function resize() {
			const width = canvas.clientWidth || 1;
			const height = canvas.clientHeight || 1;
			renderer.setSize(width, height, false);
			if (camera) camera.aspect = width / height;
			place();
		}
		const observer = new ResizeObserver(resize);
		observer.observe(canvas);
		resize();

		let previous = 0;
		let frame = requestAnimationFrame(function tick(now) {
			frame = requestAnimationFrame(tick);
			spin.advance(previous ? now - previous : 0);
			previous = now;
			group.rotation.y = spin.angle;
			if (camera) renderer.render(scene, camera);
		});

		stage = group;

		return () => {
			cancelAnimationFrame(frame);
			observer.disconnect();
			stage = null;
			camera = null;
			// Geometries/materials belong to palMeshLibrary's cache (clone() doesn't copy them); only the renderer and its GL context are ours to free.
			renderer.dispose();
			renderer.forceContextLoss();
		};
	}

	$effect(() => {
		// Cleared first so selecting a second Pal drops the first immediately instead of spinning on under the loading spinner.
		source = null;
		if (!modelUrl) {
			status = 'unavailable';
			return;
		}
		const key = characterKey;
		status = 'loading';

		const attempt = () => {
			const loaded = requestPalMesh(key);
			if (loaded) {
				source = loaded;
				status = 'ready';
				return true;
			}
			if (palMeshFailed(key)) {
				status = 'unavailable';
				return true;
			}
			return false;
		};

		if (attempt()) return;
		const off = onPalMeshLoaded(() => {
			if (attempt()) off();
		});
		return off;
	});

	$effect(() => {
		const group = stage;
		const loaded = source;
		if (!group || !loaded) return;

		// requestPalMesh returns one cached Object3D per key; an Object3D has only one parent, so it must be cloned rather than added directly.
		const model = loaded.clone();
		const bounds = palBounds(model);
		// Group rotates about its own origin, so the model must be centred on it or the Pal orbits rather than spins.
		model.position.sub(bounds.centre);
		radius = bounds.radius;
		group.add(model);
		place();

		return () => {
			group.remove(model);
		};
	});
</script>

{#if status === 'unavailable'}
	{@render fallback?.()}
{:else}
	<div class="relative size-full">
		<canvas
			{@attach mount}
			class="size-full cursor-grab touch-none active:cursor-grabbing"
			aria-label={m.pal_3d_model()}
			onpointerdown={(event) => {
				event.currentTarget.setPointerCapture(event.pointerId);
				spin.pointerDown(event.clientX, event.timeStamp);
			}}
			onpointermove={(event) => spin.pointerMove(event.clientX, event.timeStamp)}
			onpointerup={() => spin.pointerUp()}
			onpointercancel={() => spin.pointerUp()}
		></canvas>
		{#if status === 'loading'}
			<div class="absolute inset-0 flex items-center justify-center">
				<div
					class="border-surface-500 border-t-primary-500 size-8 animate-spin rounded-full border-2"
				></div>
			</div>
		{/if}
	</div>
{/if}
