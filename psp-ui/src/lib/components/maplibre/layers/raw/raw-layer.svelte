<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { AddLayerObject } from 'maplibre-gl';
	import {
		getMapContext,
		tryGetSourceContext,
		LayerContext,
		setLayerContext
	} from '../../contexts.svelte.js';
	import { generateId, addLayerEventHandler } from '../../utils.js';
	import type { RawLayerProps, LayerEventProps } from '../types.js';

	let {
		id = generateId('layer'),
		type,
		source,
		sourceLayer,
		filter,
		minzoom,
		maxzoom,
		paint = {},
		layout = {},
		beforeId,
		visible = true,
		// Mouse events
		onclick,
		ondblclick,
		onmousedown,
		onmouseup,
		onmousemove,
		onmouseenter,
		onmouseleave,
		onmouseover,
		onmouseout,
		oncontextmenu,
		// Touch events
		ontouchstart,
		ontouchend,
		ontouchcancel,
		children
	}: RawLayerProps = $props();

	const ctx = getMapContext();
	const sourceCtx = tryGetSourceContext();

	const layerId = untrack(() => id);
	const resolvedSource = untrack(() => source) ?? sourceCtx?.id;

	setLayerContext(new LayerContext(layerId, resolvedSource ?? ''));

	const cleanups: Array<() => void> = [];

	const LAYER_EVENTS: { name: keyof LayerEventProps; handler: any }[] = [];

	ctx.whenLoaded(() => {
		const initSourceLayer = untrack(() => sourceLayer);
		const initFilter = untrack(() => filter);
		const initMinzoom = untrack(() => minzoom);
		const initMaxzoom = untrack(() => maxzoom);
		const initBeforeId = untrack(() => beforeId);
		const initPaint = untrack(() => paint);
		const initLayout = untrack(() => layout);
		const initVisible = untrack(() => visible);

		const layerSpec: AddLayerObject = {
			id: layerId,
			type,
			...(resolvedSource ? { source: resolvedSource } : {}),
			...(initSourceLayer ? { 'source-layer': initSourceLayer } : {}),
			...(initFilter ? { filter: initFilter } : {}),
			...(initMinzoom !== undefined ? { minzoom: initMinzoom } : {}),
			...(initMaxzoom !== undefined ? { maxzoom: initMaxzoom } : {}),
			paint: { ...initPaint },
			layout: {
				...initLayout,
				visibility: initVisible ? 'visible' : 'none'
			}
		} as AddLayerObject;

		ctx.addLayer(layerSpec, initBeforeId);

		const map = ctx.map!;
		const eventEntries: [string, any][] = untrack(() => [
			['click', onclick],
			['dblclick', ondblclick],
			['mousedown', onmousedown],
			['mouseup', onmouseup],
			['mousemove', onmousemove],
			['mouseenter', onmouseenter],
			['mouseleave', onmouseleave],
			['mouseover', onmouseover],
			['mouseout', onmouseout],
			['contextmenu', oncontextmenu],
			['touchstart', ontouchstart],
			['touchend', ontouchend],
			['touchcancel', ontouchcancel]
		]);

		for (const [eventName, handler] of eventEntries) {
			if (handler) {
				cleanups.push(addLayerEventHandler(map, eventName, layerId, handler));
			}
		}
	});

	$effect(() => {
		if (!ctx.map || !ctx.loaded) return;
		if (!ctx.map.getLayer(layerId)) return;
		for (const [key, value] of Object.entries(paint)) {
			ctx.map.setPaintProperty(layerId, key, value);
		}
	});

	$effect(() => {
		if (!ctx.map || !ctx.loaded) return;
		if (!ctx.map.getLayer(layerId)) return;
		for (const [key, value] of Object.entries(layout)) {
			ctx.map.setLayoutProperty(layerId, key, value);
		}
	});

	$effect(() => {
		if (!ctx.map || !ctx.loaded) return;
		if (!ctx.map.getLayer(layerId)) return;
		ctx.map.setLayoutProperty(layerId, 'visibility', visible ? 'visible' : 'none');
	});

	$effect(() => {
		if (!ctx.map || !ctx.loaded) return;
		if (!ctx.map.getLayer(layerId)) return;
		ctx.map.setFilter(layerId, filter ?? null);
	});

	onDestroy(() => {
		for (const cleanup of cleanups) {
			cleanup();
		}
		ctx.removeLayer(layerId);
	});
</script>

{@render children?.()}
