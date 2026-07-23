<script lang="ts">
	import { getMapContext } from '../../contexts.svelte.js';
	import type { TooltipFeature } from '../../types.js';
	import type { TooltipProps } from './types.js';

	let { layers, offset = { x: 15, y: -15 }, content, class: className }: TooltipProps = $props();

	const ctx = getMapContext();

	let visible = $state(false);
	let x = $state(0);
	let y = $state(0);
	let feature = $state<TooltipFeature | null>(null);

	$effect(() => {
		const map = ctx.map;
		if (!map) return;

		function onMouseMove(e: maplibregl.MapMouseEvent) {
			const queryOpts = layers ? { layers } : undefined;
			const features = map!.queryRenderedFeatures(e.point, queryOpts);

			if (features.length > 0) {
				const f = features[0];
				visible = true;
				x = e.point.x;
				y = e.point.y;
				feature = {
					properties: f.properties ?? {},
					layer: f.layer?.id ?? '',
					lngLat: { lng: e.lngLat.lng, lat: e.lngLat.lat }
				};
				map!.getCanvas().style.cursor = 'pointer';
			} else {
				visible = false;
				feature = null;
				map!.getCanvas().style.cursor = '';
			}
		}

		function onMouseLeave() {
			visible = false;
			feature = null;
			map!.getCanvas().style.cursor = '';
		}

		map.on('mousemove', onMouseMove);
		map.on('mouseleave', onMouseLeave);

		return () => {
			map.off('mousemove', onMouseMove);
			map.off('mouseleave', onMouseLeave);
			map.getCanvas().style.cursor = '';
		};
	});
</script>

{#if visible && feature}
	<div
		class={className}
		style="position:absolute; left:{x + offset.x}px; top:{y +
			offset.y}px; pointer-events:none; z-index:10;"
	>
		{@render content?.(feature)}
	</div>
{/if}
