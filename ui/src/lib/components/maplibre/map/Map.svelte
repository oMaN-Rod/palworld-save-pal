<script lang="ts">
	import maplibregl from 'maplibre-gl';
	import 'maplibre-gl/dist/maplibre-gl.css';
	import { untrack } from 'svelte';
	import type { StyleSpecification } from 'maplibre-gl';
	import { MapContext, setMapContext } from '../contexts.svelte.js';
	import type { MapProps, MapEventType } from './types.js';

	let {
		style,
		transformStyle,
		center = $bindable([0, 0]),
		zoom = $bindable(1),
		bearing = $bindable(0),
		pitch = $bindable(0),
		bounds,
		maxBounds,
		minZoom = 0,
		maxZoom = 22,
		minPitch,
		maxPitch,
		projection = 'mercator',
		hash = false,
		interactive = true,
		theme = 'auto',
		map = $bindable(),

		// Accessors
		repaint,
		showCollisionBoxes,
		showOverdrawInspector,
		showPadding,
		showTileBoundaries,

		// MapOptions passthrough
		bearingSnap,
		zoomSnap,
		attributionControl,
		maplibreLogo,
		logoPosition,
		canvasContextAttributes,
		refreshExpiredTiles,
		scrollZoom,
		boxZoom,
		dragRotate,
		dragPan,
		keyboard,
		doubleClickZoom,
		touchZoomRotate,
		touchPitch,
		cooperativeGestures,
		trackResize,
		elevation,
		roll,
		renderWorldCopies,
		maxTileCacheSize,
		maxTileCacheZoomLevels,
		transformRequest,
		transformCameraUpdate,
		transformConstrain,
		locale,
		fadeDuration,
		crossSourceCollisions,
		collectResourceTiming,
		clickTolerance,
		fitBoundsOptions,
		localIdeographFontFamily,
		pitchWithRotate,
		rollEnabled,
		reduceMotion,
		pixelRatio,
		validateStyle,
		maxCanvasSize,
		cancelPendingTileRequestsWhileZooming,
		centerClampedToGround,
		aroundCenter,

		// Events — all from MapEventType
		onerror,
		onload,
		onidle,
		onremove,
		onrender,
		onresize,
		onwebglcontextlost,
		onwebglcontextrestored,
		ondataloading,
		ondata,
		ontiledataloading,
		onsourcedataloading,
		onstyledataloading,
		onsourcedata,
		onstyledata,
		onstyleimagemissing,
		ondataabort,
		onsourcedataabort,
		onboxzoomcancel,
		onboxzoomstart,
		onboxzoomend,
		ontouchcancel,
		ontouchmove,
		ontouchend,
		ontouchstart,
		onclick,
		oncontextmenu,
		ondblclick,
		onmousemove,
		onmouseup,
		onmousedown,
		onmouseout,
		onmouseover,
		onmovestart,
		onmove,
		onmoveend,
		onzoomstart,
		onzoom,
		onzoomend,
		onrotatestart,
		onrotate,
		onrotateend,
		ondragstart,
		ondrag,
		ondragend,
		onpitchstart,
		onpitch,
		onpitchend,
		onwheel,
		onterrain,

		children,
		class: className
	}: MapProps = $props();

	const ctx = new MapContext();
	setMapContext(ctx);

	let container: HTMLDivElement;
	let syncing = false;

	// Resolve effective theme
	$effect(() => {
		ctx.theme = ctx.resolveTheme(theme ?? 'auto');
	});

	// Listen for theme changes when theme is 'auto'
	$effect(() => {
		if (theme !== 'auto' || typeof window === 'undefined') return;

		// 1. Watch prefers-color-scheme media query
		const mq = window.matchMedia('(prefers-color-scheme: dark)');
		const mqHandler = (e: MediaQueryListEvent) => {
			// Only use media query if .dark class isn't controlling things
			if (!document.documentElement.classList.contains('dark')) {
				ctx.theme = e.matches ? 'dark' : 'light';
			}
		};
		mq.addEventListener('change', mqHandler);

		// 2. Watch .dark class on <html> (Tailwind / mode-watcher convention)
		//    When this fires, .dark class is authoritative — no media query fallback
		const observer = new MutationObserver(() => {
			ctx.theme = document.documentElement.classList.contains('dark') ? 'dark' : 'light';
		});
		observer.observe(document.documentElement, {
			attributes: true,
			attributeFilter: ['class']
		});

		return () => {
			mq.removeEventListener('change', mqHandler);
			observer.disconnect();
		};
	});

	// All event names to forward
	const EVENT_NAMES: (keyof MapEventType)[] = [
		'error',
		'idle',
		'remove',
		'render',
		'resize',
		'webglcontextlost',
		'webglcontextrestored',
		'dataloading',
		'data',
		'tiledataloading',
		'sourcedataloading',
		'styledataloading',
		'sourcedata',
		'styledata',
		'styleimagemissing',
		'dataabort',
		'sourcedataabort',
		'boxzoomcancel',
		'boxzoomstart',
		'boxzoomend',
		'touchcancel',
		'touchmove',
		'touchend',
		'touchstart',
		'click',
		'contextmenu',
		'dblclick',
		'mousemove',
		'mouseup',
		'mousedown',
		'mouseout',
		'mouseover',
		'movestart',
		'move',
		'moveend',
		'zoomstart',
		'zoom',
		'zoomend',
		'rotatestart',
		'rotate',
		'rotateend',
		'dragstart',
		'drag',
		'dragend',
		'pitchstart',
		'pitch',
		'pitchend',
		'wheel',
		'terrain'
	];

	// Collect event props into a lookup for registration
	function getEventHandlers(): Partial<Record<keyof MapEventType, (ev: any) => void>> {
		return {
			error: onerror as any,
			idle: onidle,
			remove: onremove,
			render: onrender,
			resize: onresize,
			webglcontextlost: onwebglcontextlost,
			webglcontextrestored: onwebglcontextrestored,
			dataloading: ondataloading,
			data: ondata,
			tiledataloading: ontiledataloading,
			sourcedataloading: onsourcedataloading,
			styledataloading: onstyledataloading,
			sourcedata: onsourcedata,
			styledata: onstyledata,
			styleimagemissing: onstyleimagemissing,
			dataabort: ondataabort,
			sourcedataabort: onsourcedataabort,
			boxzoomcancel: onboxzoomcancel,
			boxzoomstart: onboxzoomstart,
			boxzoomend: onboxzoomend,
			touchcancel: ontouchcancel,
			touchmove: ontouchmove,
			touchend: ontouchend,
			touchstart: ontouchstart,
			click: onclick,
			contextmenu: oncontextmenu,
			dblclick: ondblclick,
			mousemove: onmousemove,
			mouseup: onmouseup,
			mousedown: onmousedown,
			mouseout: onmouseout,
			mouseover: onmouseover,
			movestart: onmovestart,
			move: onmove,
			moveend: onmoveend,
			zoomstart: onzoomstart,
			zoom: onzoom,
			zoomend: onzoomend,
			rotatestart: onrotatestart,
			rotate: onrotate,
			rotateend: onrotateend,
			dragstart: ondragstart,
			drag: ondrag,
			dragend: ondragend,
			pitchstart: onpitchstart,
			pitch: onpitch,
			pitchend: onpitchend,
			wheel: onwheel,
			terrain: onterrain
		};
	}

	// Create the map instance (only depends on `container`)
	$effect(() => {
		if (!container) return;

		const mapInstance = untrack(() => {
			const opts: maplibregl.MapOptions = {
				container,
				style: applyTransform(style),
				center,
				zoom,
				bearing,
				pitch,
				maxBounds,
				minZoom,
				maxZoom,
				hash,
				interactive,
				...(bounds ? { bounds } : {}),
				...(minPitch != null ? { minPitch } : {}),
				...(maxPitch != null ? { maxPitch } : {}),
				...(bearingSnap != null ? { bearingSnap } : {}),
				...(zoomSnap != null ? { zoomSnap } : {}),
				...(attributionControl != null ? { attributionControl } : {}),
				...(maplibreLogo != null ? { maplibreLogo } : {}),
				...(logoPosition != null ? { logoPosition } : {}),
				...(canvasContextAttributes != null ? { canvasContextAttributes } : {}),
				...(refreshExpiredTiles != null ? { refreshExpiredTiles } : {}),
				...(scrollZoom != null ? { scrollZoom } : {}),
				...(boxZoom != null ? { boxZoom } : {}),
				...(dragRotate != null ? { dragRotate } : {}),
				...(dragPan != null ? { dragPan } : {}),
				...(keyboard != null ? { keyboard } : {}),
				...(doubleClickZoom != null ? { doubleClickZoom } : {}),
				...(touchZoomRotate != null ? { touchZoomRotate } : {}),
				...(touchPitch != null ? { touchPitch } : {}),
				...(cooperativeGestures != null ? { cooperativeGestures } : {}),
				...(trackResize != null ? { trackResize } : {}),
				...(elevation != null ? { elevation } : {}),
				...(roll != null ? { roll } : {}),
				...(renderWorldCopies != null ? { renderWorldCopies } : {}),
				...(maxTileCacheSize != null ? { maxTileCacheSize } : {}),
				...(maxTileCacheZoomLevels != null ? { maxTileCacheZoomLevels } : {}),
				...(transformRequest != null ? { transformRequest } : {}),
				...(transformCameraUpdate != null ? { transformCameraUpdate } : {}),
				...(transformConstrain != null ? { transformConstrain } : {}),
				...(locale != null ? { locale } : {}),
				...(fadeDuration != null ? { fadeDuration } : {}),
				...(crossSourceCollisions != null ? { crossSourceCollisions } : {}),
				...(collectResourceTiming != null ? { collectResourceTiming } : {}),
				...(clickTolerance != null ? { clickTolerance } : {}),
				...(fitBoundsOptions != null ? { fitBoundsOptions } : {}),
				...(localIdeographFontFamily != null ? { localIdeographFontFamily } : {}),
				...(pitchWithRotate != null ? { pitchWithRotate } : {}),
				...(rollEnabled != null ? { rollEnabled } : {}),
				...(reduceMotion != null ? { reduceMotion } : {}),
				...(pixelRatio != null ? { pixelRatio } : {}),
				...(validateStyle != null ? { validateStyle } : {}),
				...(maxCanvasSize != null ? { maxCanvasSize } : {}),
				...(cancelPendingTileRequestsWhileZooming != null
					? { cancelPendingTileRequestsWhileZooming }
					: {}),
				...(centerClampedToGround != null ? { centerClampedToGround } : {}),
				...(aroundCenter != null ? { aroundCenter } : {})
			};
			return new maplibregl.Map(opts);
		});

		ctx.map = mapInstance;
		map = mapInstance;

		mapInstance.on('load', (e) => {
			// Projection must be set after the style loads
			const proj = untrack(() => projection);
			if (proj !== 'mercator') {
				mapInstance.setProjection({ type: proj });
			}

			// Apply accessor properties
			const initRepaint = untrack(() => repaint);
			const initShowCollisionBoxes = untrack(() => showCollisionBoxes);
			const initShowOverdrawInspector = untrack(() => showOverdrawInspector);
			const initShowPadding = untrack(() => showPadding);
			const initShowTileBoundaries = untrack(() => showTileBoundaries);

			if (initRepaint != null) mapInstance.repaint = initRepaint;
			if (initShowCollisionBoxes != null) mapInstance.showCollisionBoxes = initShowCollisionBoxes;
			if (initShowOverdrawInspector != null)
				mapInstance.showOverdrawInspector = initShowOverdrawInspector;
			if (initShowPadding != null) mapInstance.showPadding = initShowPadding;
			if (initShowTileBoundaries != null) mapInstance.showTileBoundaries = initShowTileBoundaries;

			ctx.markLoaded();
			onload?.(e);
		});

		// Bidirectional sync: map -> props
		mapInstance.on('move', () => {
			syncFromMap();
		});

		mapInstance.on('zoom', () => {
			syncFromMap();
		});

		// Register all event handlers (except 'load' which is handled above)
		const handlers = untrack(() => getEventHandlers());
		const cleanups: Array<() => void> = [];

		for (const name of EVENT_NAMES) {
			if (name === 'load') continue; // handled above
			const handler = handlers[name];
			if (handler) {
				mapInstance.on(name as any, handler as any);
				cleanups.push(() => mapInstance.off(name as any, handler as any));
			}
		}

		return () => {
			for (const cleanup of cleanups) cleanup();
			ctx.cleanup();
			map = undefined;
		};
	});

	// Bidirectional sync: props -> map
	$effect(() => {
		if (!ctx.map || syncing) return;
		const m = ctx.map;
		const mc = m.getCenter();
		const [lng, lat] = center;
		if (Math.abs(mc.lng - lng) > 1e-6 || Math.abs(mc.lat - lat) > 1e-6) {
			m.setCenter(center);
		}
	});

	$effect(() => {
		if (!ctx.map || syncing) return;
		if (Math.abs(ctx.map.getZoom() - zoom) > 1e-6) {
			ctx.map.setZoom(zoom);
		}
	});

	$effect(() => {
		if (!ctx.map || syncing) return;
		if (Math.abs(ctx.map.getBearing() - bearing) > 1e-3) {
			ctx.map.setBearing(bearing);
		}
	});

	$effect(() => {
		if (!ctx.map || syncing) return;
		if (Math.abs(ctx.map.getPitch() - pitch) > 1e-3) {
			ctx.map.setPitch(pitch);
		}
	});

	// Reactive accessor properties
	$effect(() => {
		if (!ctx.map) return;
		if (repaint != null) ctx.map.repaint = repaint;
	});

	$effect(() => {
		if (!ctx.map) return;
		if (showCollisionBoxes != null) ctx.map.showCollisionBoxes = showCollisionBoxes;
	});

	$effect(() => {
		if (!ctx.map) return;
		if (showOverdrawInspector != null) ctx.map.showOverdrawInspector = showOverdrawInspector;
	});

	$effect(() => {
		if (!ctx.map) return;
		if (showPadding != null) ctx.map.showPadding = showPadding;
	});

	$effect(() => {
		if (!ctx.map) return;
		if (showTileBoundaries != null) ctx.map.showTileBoundaries = showTileBoundaries;
	});

	// Style changes (after initial creation)
	let prevStyle: string | StyleSpecification | undefined;
	$effect(() => {
		const newStyle = applyTransform(style);
		if (prevStyle === undefined) {
			prevStyle = newStyle;
			return;
		}
		if (ctx.map && newStyle !== prevStyle) {
			prevStyle = newStyle;
			ctx.markUnloaded();
			ctx.map.setStyle(newStyle);
			ctx.map.once('idle', () => {
				ctx.markLoaded();
			});
		}
	});

	// Every write here must be conditional. MapLibre fires `move` even when a camera
	// call changed nothing (jumpTo fires it unconditionally), so an unconditional
	// `center = [c.lng, c.lat]` allocates a fresh array on each no-op event and
	// registers as a state change, re-entering any effect bound to it.
	function syncFromMap() {
		if (!ctx.map) return;
		syncing = true;
		const c = ctx.map.getCenter();
		if (center[0] !== c.lng || center[1] !== c.lat) center = [c.lng, c.lat];
		const z = ctx.map.getZoom();
		if (zoom !== z) zoom = z;
		const b = ctx.map.getBearing();
		if (bearing !== b) bearing = b;
		const p = ctx.map.getPitch();
		if (pitch !== p) pitch = p;
		syncing = false;
	}

	function applyTransform(s: string | StyleSpecification): string | StyleSpecification {
		if (transformStyle && typeof s === 'object') {
			return transformStyle(s);
		}
		return s;
	}
</script>

<div
	bind:this={container}
	class={className}
	data-svlibre-theme={ctx.theme}
	style="width: 100%; height: 100%;"
>
	{#if ctx.loaded}
		{@render children?.()}
	{/if}
</div>

<style>
	/* ── Theme CSS custom properties ── */
	div[data-svlibre-theme='light'] {
		--svlibre-ctrl-bg: #fff;
		--svlibre-ctrl-bg-hover: rgba(0, 0, 0, 0.05);
		--svlibre-ctrl-color: #333;
		--svlibre-ctrl-border: rgba(0, 0, 0, 0.1);
		--svlibre-ctrl-shadow: 0 0 0 2px rgba(0, 0, 0, 0.1);
		--svlibre-tooltip-bg: rgba(0, 0, 0, 0.8);
		--svlibre-tooltip-color: #fff;
	}

	div[data-svlibre-theme='dark'] {
		--svlibre-ctrl-bg: #181818;
		--svlibre-ctrl-bg-hover: rgba(255, 255, 255, 0.1);
		--svlibre-ctrl-color: #e5e7eb;
		--svlibre-ctrl-border: rgba(255, 255, 255, 0.15);
		--svlibre-ctrl-shadow: 0 0 0 2px rgba(0, 0, 0, 0.3);
		--svlibre-tooltip-bg: rgba(30, 30, 30, 0.9);
		--svlibre-tooltip-color: #e5e7eb;
	}

	/* ── Dark mode overrides for native MapLibre controls ── */

	div[data-svlibre-theme='dark'] :global(.maplibregl-ctrl-group) {
		background: var(--svlibre-ctrl-bg);
		box-shadow: var(--svlibre-ctrl-shadow);
	}

	div[data-svlibre-theme='dark'] :global(.maplibregl-ctrl-group button) {
		background-color: transparent;
	}

	div[data-svlibre-theme='dark'] :global(.maplibregl-ctrl-group button + button) {
		border-top-color: var(--svlibre-ctrl-border);
	}

	div[data-svlibre-theme='dark'] :global(.maplibregl-ctrl-group button:hover) {
		background-color: var(--svlibre-ctrl-bg-hover);
	}

	/* Navigation control icons */
	div[data-svlibre-theme='dark']
		:global(.maplibregl-ctrl button.maplibregl-ctrl-fullscreen .maplibregl-ctrl-icon) {
		filter: invert(1);
	}

	div[data-svlibre-theme='dark']
		:global(.maplibregl-ctrl button.maplibregl-ctrl-compass .maplibregl-ctrl-icon) {
		background-image: url('data:image/svg+xml;charset=utf-8,%3Csvg%20xmlns%3D%22http%3A//www.w3.org/2000/svg%22%20width%3D%2229%22%20height%3D%2229%22%20fill%3D%22white%22%20viewBox%3D%220%200%2029%2029%22%3E%3Cpath%20d%3D%22m10.5%2014%204-8%204%208z%22/%3E%3Cpath%20fill%3D%22%23ff0000%22%20d%3D%22m10.5%2016%204%208%204-8z%22/%3E%3C/svg%3E');
	}

	/* Scale control */
	div[data-svlibre-theme='dark'] :global(.maplibregl-ctrl-scale) {
		background-color: var(--svlibre-ctrl-bg);
		color: var(--svlibre-ctrl-color);
		border-color: var(--svlibre-ctrl-color);
		box-shadow: var(--svlibre-ctrl-shadow);
	}

	/* Attribution control */
	div[data-svlibre-theme='dark'] :global(.maplibregl-ctrl-attrib) {
		background-color: var(--svlibre-ctrl-bg);
		color: var(--svlibre-ctrl-color);
	}

	div[data-svlibre-theme='dark'] :global(.maplibregl-ctrl-attrib a) {
		color: #93c5fd;
	}

	div[data-svlibre-theme='dark']
		:global(.maplibregl-ctrl-attrib button.maplibregl-ctrl-attrib-button) {
		filter: invert(1);
	}
</style>
