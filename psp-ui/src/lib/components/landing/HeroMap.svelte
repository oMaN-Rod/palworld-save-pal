<script lang="ts">
	import { onMount } from 'svelte';
	import 'maplibre-gl/dist/maplibre-gl.css';
	import type maplibregl from 'maplibre-gl';
	import type { StyleSpecification } from 'maplibre-gl';
	import { Map as MLMap, Source, Layer, Terrain } from '$components/maplibre';
	import { MAP_MAX_BOUNDS, verticalScaleFactor } from '$components/map/geo/mercator';
	import { cmPerPx } from '$components/map/geo/utils';

	const EMPTY_STYLE: StyleSpecification = {
		version: 8,
		sources: {},
		// MapLibre WebGL paint property — can't take CSS custom properties, only
		// concrete hex. Visible only before raster tiles load.
		layers: [{ id: 'background', type: 'background', paint: { 'background-color': '#0a1220' } }]
	};

	let map = $state<maplibregl.Map | undefined>();
	let failed = $state(false);
	let center = $state<[number, number]>([0, 0]);
	let zoom = $state(2.6);
	// 3D terrain (DEM) is GPU-heavy; default off below md (768px) so phones don't pay the cost. Toggle still opts in.
	const isMobile =
		typeof window !== 'undefined' &&
		typeof window.matchMedia === 'function' &&
		window.matchMedia('(max-width: 767px)').matches;
	let pitch = $state(isMobile ? 0 : 52);
	let show3d = $state(!isMobile);

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
	let lastTs = 0;
	const DEG_PER_MS = 0.0012;
	const MAX_DT = 100;

	function startOrbit() {
		if (reduceMotion || !map) return;
		lastTs = 0;
		const step = (ts: number) => {
			if (!map || failed) {
				raf = 0;
				return;
			}
			if (lastTs === 0) lastTs = ts;
			const dt = Math.min(ts - lastTs, MAX_DT);
			lastTs = ts;
			map.setBearing((map.getBearing() + dt * DEG_PER_MS) % 360);
			raf = requestAnimationFrame(step);
		};
		raf = requestAnimationFrame(step);
	}

	function handleLoad() {
		if (!map) return;
		startOrbit();
	}

	function toggle3d() {
		show3d = !show3d;
		pitch = show3d ? 52 : 0;
	}

	onMount(() => {
		return () => {
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
			interactive={false}
			attributionControl={false}
			onload={handleLoad}
			onwebglcontextlost={() => (failed = true)}
		>
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
			radial-gradient(
				200px 150px at 26% 40%,
				color-mix(in srgb, var(--color-success-700) 70%, transparent),
				transparent 68%
			),
			radial-gradient(
				240px 180px at 76% 66%,
				color-mix(in srgb, var(--color-success-600) 70%, transparent),
				transparent 70%
			),
			radial-gradient(
				150px 120px at 60% 20%,
				color-mix(in srgb, var(--color-success-500) 60%, transparent),
				transparent 68%
			),
			linear-gradient(160deg, var(--color-surface-900), var(--color-surface-950) 74%);
	}
	.hero-3d-toggle {
		background: color-mix(in srgb, var(--color-primary-500) 90%, transparent);
		color: var(--color-surface-50);
	}
</style>
