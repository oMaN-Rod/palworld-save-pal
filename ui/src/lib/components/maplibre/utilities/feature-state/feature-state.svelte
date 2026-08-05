<script lang="ts">
	import { untrack } from 'svelte';
	import { getMapContext } from '../../contexts.svelte.js';
	import type { FeatureIdentifier } from 'maplibre-gl';
	import type { FeatureStateProps } from './types.js';

	let { source, sourceLayer, id, state }: FeatureStateProps = $props();

	const ctx = getMapContext();

	// Feature ids are unique only within a source, so the previously-applied
	// target must be tracked as (source, sourceLayer, id), not id alone.
	let lastApplied: { source: string; sourceLayer?: string; id: string | number } | null = null;

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

		const prevTarget = untrack(() => lastApplied);

		const targetChanged =
			prevTarget !== null &&
			(prevTarget.source !== stableSource ||
				prevTarget.sourceLayer !== stableSourceLayer ||
				prevTarget.id !== currentId);

		// Clear previous state by setting all keys to null/false (preserves other components' state)
		if (targetChanged) {
			try {
				const resetState: Record<string, unknown> = {};
				const prevState = untrack(() => state);
				for (const key of Object.keys(prevState)) {
					resetState[key] = null;
				}
				const prevIdentifier: FeatureIdentifier = {
					source: prevTarget!.source,
					id: prevTarget!.id
				};
				if (prevTarget!.sourceLayer) prevIdentifier.sourceLayer = prevTarget!.sourceLayer;
				map.setFeatureState(prevIdentifier, resetState);
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

		lastApplied =
			currentId !== null && currentId !== undefined
				? { source: stableSource, sourceLayer: stableSourceLayer, id: currentId }
				: null;

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
