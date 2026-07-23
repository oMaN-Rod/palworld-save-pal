<script lang="ts">
	import maplibregl from 'maplibre-gl';
	import { untrack } from 'svelte';
	import { getMapContext } from '../../contexts.svelte.js';
	import type { FullscreenControlProps } from './types.js';

	let {
		position = 'top-right',
		container,
		pseudo,
		onfullscreenstart,
		onfullscreenend
	}: FullscreenControlProps = $props();

	const ctx = getMapContext();

	$effect(() => {
		const map = ctx.map;
		if (!map) return;

		const opts = untrack(() => {
			const o: Record<string, unknown> = {};
			if (container) o.container = container;
			if (pseudo != null) o.pseudo = pseudo;
			return o;
		});

		const control = new maplibregl.FullscreenControl(opts as any);

		const startCb = untrack(() => onfullscreenstart);
		const endCb = untrack(() => onfullscreenend);

		if (startCb) (control as any).on('fullscreenstart', startCb);
		if (endCb) (control as any).on('fullscreenend', endCb);

		ctx.addControl(
			control,
			untrack(() => position)
		);

		return () => {
			ctx.removeControl(control);
		};
	});
</script>
