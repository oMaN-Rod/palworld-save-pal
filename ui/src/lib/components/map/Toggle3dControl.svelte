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

<ToggleIconControl {active} {title} {position} {onchange} buttonClass="maplibregl-ctrl-3d" />

<style>
	:global(.maplibregl-ctrl button.maplibregl-ctrl-3d .maplibregl-ctrl-icon) {
		background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='29' height='29' viewBox='-2.5 -2.5 29 29' fill='none' stroke='%23333' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z'/%3E%3Cpath d='m3.3 7 8.7 5 8.7-5'/%3E%3Cpath d='M12 22V12'/%3E%3C/svg%3E");
	}

	/* Matches maplibre's own convention for an inactive icon. Theme inversion is
	   handled centrally by the map wrapper, alongside the built-in controls. */
	:global(.maplibregl-ctrl button.maplibregl-ctrl-3d:not(.is-active) .maplibregl-ctrl-icon) {
		opacity: 0.25;
	}
</style>
