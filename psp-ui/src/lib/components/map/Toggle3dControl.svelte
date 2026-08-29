<script lang="ts">
	import type { ControlPosition } from '$components/maplibre';
	import ToggleIconControl from './ToggleIconControl.svelte';

	let {
		active = false,
		title,
		position = 'top-right',
		onchange
	}: {
		active?: boolean;
		title: string;
		position?: ControlPosition;
		onchange?: () => void;
	} = $props();
</script>

<ToggleIconControl
	{active}
	{title}
	{position}
	{onchange}
	label={active ? '3D' : '2D'}
	buttonClass="maplibregl-ctrl-3d"
/>

<style>
	/* Matches the SVG icons' stroke='#333' convention so the existing dark-theme
	   filter: invert(1) rule (applied centrally by the map wrapper) flips it
	   correctly, instead of inheriting the app's unrelated body text color. */
	:global(.maplibregl-ctrl button.maplibregl-ctrl-3d .maplibregl-ctrl-icon-text) {
		display: flex;
		align-items: center;
		justify-content: center;
		color: #333;
		font-size: 0.75rem;
		font-weight: 700;
		letter-spacing: 0.02em;
		line-height: 1;
	}

	/* Raised from the icon variant's 0.25: at #333-on-white, 0.65 opacity only
	   reaches ~4.30:1, short of the 4.5:1 text minimum. 0.75 clears it in both
	   themes with margin. */
	:global(.maplibregl-ctrl button.maplibregl-ctrl-3d:not(.is-active) .maplibregl-ctrl-icon-text) {
		opacity: 0.75;
	}
</style>
