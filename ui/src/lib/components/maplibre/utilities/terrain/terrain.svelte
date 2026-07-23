<script lang="ts">
	import { untrack } from 'svelte';
	import { getMapContext } from '../../contexts.svelte.js';
	import type { TerrainProps } from './types.js';

	// Below this relative change the visual difference is imperceptible, so latitude-driven
	// jitter is dropped rather than paid for.
	const EXAGGERATION_EPSILON = 0.005;

	let { source, exaggeration = 1 }: TerrainProps = $props();

	const ctx = getMapContext();

	let applied = untrack(() => exaggeration);

	$effect(() => {
		const map = ctx.map;
		if (!map || !ctx.loaded) return;

		const initial = untrack(() => exaggeration);

		try {
			map.setTerrain({ source, exaggeration: initial });
			applied = initial;
		} catch {
			// Source may not be registered yet; a later run of this effect retries.
		}

		return () => {
			try {
				map.setTerrain(null);
			} catch {
				// Map may already be destroyed.
			}
		};
	});

	$effect(() => {
		const next = exaggeration;
		const map = ctx.map;
		if (!map || !ctx.loaded) return;

		const terrain = map.terrain;
		if (!terrain) return;
		if (Math.abs(next - applied) <= Math.abs(applied) * EXAGGERATION_EPSILON) return;

		applied = next;
		terrain.exaggeration = next;
		terrain.options.exaggeration = next;
		map.triggerRepaint();
	});
</script>
