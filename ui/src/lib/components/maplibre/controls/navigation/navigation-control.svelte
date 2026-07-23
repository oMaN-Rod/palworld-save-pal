<script lang="ts">
	import maplibregl from 'maplibre-gl';
	import { untrack } from 'svelte';
	import { getMapContext } from '../../contexts.svelte.js';
	import type { NavigationControlProps } from './types.js';

	let {
		position = 'top-right',
		showCompass = true,
		showZoom = true,
		visualizePitch = false,
		visualizeRoll
	}: NavigationControlProps = $props();

	const ctx = getMapContext();

	$effect(() => {
		const map = ctx.map;
		if (!map) return;

		const control = untrack(() => {
			const opts: maplibregl.NavigationControlOptions = {
				showCompass,
				showZoom,
				visualizePitch
			};
			if (visualizeRoll != null) (opts as any).visualizeRoll = visualizeRoll;
			return new maplibregl.NavigationControl(opts);
		});

		ctx.addControl(
			control,
			untrack(() => position)
		);

		return () => {
			ctx.removeControl(control);
		};
	});
</script>
