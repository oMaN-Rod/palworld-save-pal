<script lang="ts">
	import { onMount } from 'svelte';
	import 'maplibre-gl/dist/maplibre-gl.css';
	import type maplibregl from 'maplibre-gl';
	import type { StyleSpecification } from 'maplibre-gl';
	import { Map as MLMap, Source, Layer, Terrain, Control } from '$components/maplibre';
	import { MAP_MAX_BOUNDS, verticalScaleFactor } from '$components/map/mercator';
	import { cmPerPx } from '$components/map/utils';

	const EMPTY_STYLE: StyleSpecification = {
		version: 8,
		sources: {},
		layers: [{ id: 'background', type: 'background', paint: { 'background-color': '#0a1220' } }]
	};

	let map = $state<maplibregl.Map | undefined>();
	let failed = $state(false);
	let center = $state<[number, number]>([0, 0]);
	let zoom = $state(2.6);
	let pitch = $state(52);
	const show3d = $derived(pitch > 5);

	const verticalScale = verticalScaleFactor(0, cmPerPx('MainMap'));

	const reduceMotion =
		typeof window !== 'undefined' &&
		typeof window.matchMedia === 'function' &&
		window.matchMedia('(prefers-reduced-motion: reduce)').matches;

	function webglAvailable(): boolean {
		if (typeof document === 'undefined') return false;
		try {
			const c = document.createElement('canvas');
			return !!(c.getContext('webgl2') || c.getContext('webgl'));
		} catch {
			return false;
		}
	}
	const canRender = webglAvailable();

	let raf = 0;
	let paused = false;
	let lastTs = 0;
	const DEG_PER_MS = 0.0012;

	function startOrbit() {
		if (reduceMotion || !map) return;
		lastTs = 0;
		const step = (ts: number) => {
			if (!map || failed) {
				raf = 0;
				return;
			}
			if (lastTs === 0) lastTs = ts;
			const dt = ts - lastTs;
			lastTs = ts;
			if (!paused) map.setBearing((map.getBearing() + dt * DEG_PER_MS) % 360);
			raf = requestAnimationFrame(step);
		};
		raf = requestAnimationFrame(step);
	}

	function handleLoad() {
		if (!map) return;
		map.on('dragstart', () => (paused = true));
		map.on('dragend', () => (paused = false));
		startOrbit();
	}

	function toggle3d() {
		pitch = show3d ? 0 : 52;
	}

	onMount(() => {
		const onVisibility = () => (paused = document.hidden);
		document.addEventListener('visibilitychange', onVisibility);
		return () => {
			document.removeEventListener('visibilitychange', onVisibility);
			if (raf) cancelAnimationFrame(raf);
		};
	});
</script>

<div class="relative h-full w-full">
	<div class="hero-poster absolute inset-0"></div>
	{#if canRender && !failed}
		<MLMap
			bind:map
			bind:center
			bind:zoom
			bind:pitch
			class="absolute inset-0 h-full w-full"
			style={EMPTY_STYLE}
			minZoom={1}
			maxZoom={6}
			maxBounds={MAP_MAX_BOUNDS}
			renderWorldCopies={false}
			dragRotate={true}
			pitchWithRotate={true}
			touchZoomRotate={true}
			attributionControl={false}
			onload={handleLoad}
			onwebglcontextlost={() => (failed = true)}
		>
			<Control.Navigation position="top-right" showCompass={false} />
			<Source.Raster tiles={['/maps/mainmap/{z}/{x}/{y}.webp']} tileSize={512} maxzoom={4}>
				<Layer.Raster paint={{ 'raster-fade-duration': 300 }} />
			</Source.Raster>
			{#if show3d}
				<Source.RasterDEM
					id="dem-mainmap"
					tiles={['/maps/dem/mainmap/{z}/{x}/{y}.png']}
					tileSize={512}
					maxzoom={4}
					encoding="custom"
					redFactor={512}
					greenFactor={2}
					blueFactor={0}
					baseShift={50000}
				>
					<Terrain source="dem-mainmap" exaggeration={verticalScale} />
				</Source.RasterDEM>
			{/if}
		</MLMap>
		<button
			type="button"
			class="hero-3d-toggle absolute bottom-3 left-3 z-10 rounded-full px-3 py-1 text-xs font-semibold"
			onclick={toggle3d}
		>
			{show3d ? '3D on' : '3D off'}
		</button>
	{/if}
</div>

<style>
	.hero-poster {
		background:
			radial-gradient(200px 150px at 26% 40%, #2f6b3a, transparent 68%),
			radial-gradient(240px 180px at 76% 66%, #3b7d47, transparent 70%),
			radial-gradient(150px 120px at 60% 20%, #6b8f3a, transparent 68%),
			linear-gradient(160deg, #0b2a3a, #08101a 74%);
	}
	.hero-3d-toggle {
		background: rgba(14, 165, 233, 0.9);
		color: #04121c;
	}
</style>
