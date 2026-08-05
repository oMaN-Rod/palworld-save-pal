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

<ToggleIconControl {active} {title} {position} {onchange} buttonClass="maplibregl-ctrl-detailed" />

<style>
	/* lucide `boxes`, the icon this toggle used while it lived in the options panel. */
	:global(.maplibregl-ctrl button.maplibregl-ctrl-detailed .maplibregl-ctrl-icon) {
		background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='29' height='29' viewBox='-2.5 -2.5 29 29' fill='none' stroke='%23333' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M2.97 12.92A2 2 0 0 0 2 14.63v3.24a2 2 0 0 0 .97 1.71l3 1.8a2 2 0 0 0 2.06 0L12 19v-5.5l-5-3-4.03 2.42Z'/%3E%3Cpath d='m7 16.5-4.74-2.85'/%3E%3Cpath d='m7 16.5 5-3'/%3E%3Cpath d='M7 16.5v5.17'/%3E%3Cpath d='M12 13.5V19l3.97 2.38a2 2 0 0 0 2.06 0l3-1.8a2 2 0 0 0 .97-1.71v-3.24a2 2 0 0 0-.97-1.71L17 10.5l-5 3Z'/%3E%3Cpath d='m17 16.5-5-3'/%3E%3Cpath d='m17 16.5 4.74-2.85'/%3E%3Cpath d='M17 16.5v5.17'/%3E%3Cpath d='M7.97 4.42A2 2 0 0 0 7 6.13v4.37l5 3 5-3V6.13a2 2 0 0 0-.97-1.71l-3-1.8a2 2 0 0 0-2.06 0l-3 1.8Z'/%3E%3Cpath d='M12 8 7.26 5.15'/%3E%3Cpath d='m12 8 4.74-2.85'/%3E%3Cpath d='M12 13.5V8'/%3E%3C/svg%3E");
	}

	/* Matches maplibre's own convention for an inactive icon. Theme inversion is
	   handled centrally by the map wrapper, alongside the built-in controls. */
	:global(.maplibregl-ctrl button.maplibregl-ctrl-detailed:not(.is-active) .maplibregl-ctrl-icon) {
		opacity: 0.25;
	}
</style>
