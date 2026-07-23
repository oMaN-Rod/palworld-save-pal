<script lang="ts">
	import { untrack } from 'svelte';
	import { getMapContext } from '../../contexts.svelte.js';
	import type { FeatureIdentifier } from 'maplibre-gl';
	import type { FeatureStateProps } from './types.js';

	let { source, sourceLayer, id, state }: FeatureStateProps = $props();

	const ctx = getMapContext();

	let lastAppliedId: string | number | null = null;

	$effect(() => {
		const map = ctx.map;
		const loaded = ctx.loaded;
		if (!map || !loaded) return;

		const stableSource = untrack(() => source);
		const stableSourceLayer = untrack(() => sourceLayer);

		const currentId = id;
		const currentState = state;

		function makeTarget(featureId: string | number): FeatureIdentifier {
			const target: FeatureIdentifier = { source: stableSource, id: featureId };
			if (stableSourceLayer) target.sourceLayer = stableSourceLayer;
			return target;
		}

		const prevId = untrack(() => lastAppliedId);

		// Clear previous state by setting all keys to null/false (preserves other components' state)
		if (prevId !== null && prevId !== undefined && prevId !== currentId) {
			try {
				const resetState: Record<string, unknown> = {};
				const prevState = untrack(() => state);
				for (const key of Object.keys(prevState)) {
					resetState[key] = null;
				}
				map.setFeatureState(makeTarget(prevId), resetState);
			} catch {
				// Source may not exist yet or feature may be gone
			}
		}

		if (currentId !== null && currentId !== undefined) {
			try {
				map.setFeatureState(makeTarget(currentId), currentState);
			} catch {
				// Source may not exist yet
			}
		}

		lastAppliedId = currentId;

		return () => {
			if (currentId !== null && currentId !== undefined) {
				try {
					const resetState: Record<string, unknown> = {};
					for (const key of Object.keys(currentState)) {
						resetState[key] = null;
					}
					map.setFeatureState(makeTarget(currentId), resetState);
				} catch {
					// Map may be destroyed
				}
			}
		};
	});
</script>
