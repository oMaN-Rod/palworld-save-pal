<script lang="ts">
	import type { RenderFpsSample } from './fpsMonitor';

	let { sample }: { sample: RenderFpsSample } = $props();

	// Color-banded like every other FPS readout users know: green is healthy,
	// amber is playable but visibly stuttery on a map pan, red is struggling.
	const tone = $derived(
		!sample.rendered || sample.fps >= 50
			? 'text-emerald-400'
			: sample.fps >= 30
				? 'text-amber-400'
				: 'text-red-400'
	);
</script>

<!-- Mirrors the coordinate readout's chrome so the two read like a pair. -->
<div class="fps-display" role="status" aria-live="off">
	<span class="fps-label">FPS</span>
	<span class="fps-value {tone}" data-testid="map-fps-value">
		{#if sample.rendered}
			{Math.round(sample.fps)}
		{:else}
			&ndash;
		{/if}
	</span>
</div>

<style>
	.fps-display {
		position: absolute;
		bottom: 8px;
		left: 8px;
		display: flex;
		align-items: baseline;
		gap: 6px;
		background: color-mix(in srgb, var(--color-surface-900) 85%, transparent);
		backdrop-filter: blur(8px);
		border: 1px solid color-mix(in srgb, var(--color-surface-700) 40%, transparent);
		color: white;
		padding: 5px 10px;
		border-radius: 6px;
		font-size: 11px;
		font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
		line-height: 1.1;
		pointer-events: none;
		z-index: 500;
	}
	.fps-label {
		font-size: 9px;
		letter-spacing: 0.08em;
		opacity: 0.7;
	}
	.fps-value {
		font-size: 14px;
		font-variant-numeric: tabular-nums;
		min-width: 2ch;
		text-align: right;
	}
</style>
