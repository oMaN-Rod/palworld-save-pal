<script lang="ts">
	import type { IControl } from 'maplibre-gl';
	import { untrack } from 'svelte';
	import { getMapContext, type ControlPosition } from '$components/maplibre';

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

	const ctx = getMapContext();

	// Read through a holder so a new inline callback identity cannot re-create
	// the control on every parent render.
	const latest: { onchange?: () => void } = { onchange: untrack(() => onchange) };
	$effect(() => {
		latest.onchange = onchange;
	});

	let button = $state<HTMLButtonElement>();

	$effect(() => {
		const map = ctx.map;
		if (!map) return;

		// Mirrors NavigationControl/FullscreenControl: a ctrl-group div holding a
		// button whose only child is an empty span.maplibregl-ctrl-icon. Sizing,
		// hover, focus and the divider between stacked buttons all come from
		// maplibre's own stylesheet once the markup matches.
		const container = document.createElement('div');
		container.className = 'maplibregl-ctrl maplibregl-ctrl-group';

		const el = document.createElement('button');
		el.type = 'button';
		el.className = 'maplibregl-ctrl-3d';

		const icon = document.createElement('span');
		icon.className = 'maplibregl-ctrl-icon';
		icon.setAttribute('aria-hidden', 'true');
		el.appendChild(icon);
		container.appendChild(el);

		const handleClick = () => latest.onchange?.();
		el.addEventListener('click', handleClick);

		const control: IControl = {
			onAdd: () => container,
			onRemove: () => container.remove()
		};
		ctx.addControl(
			control,
			untrack(() => position)
		);
		button = el;

		return () => {
			el.removeEventListener('click', handleClick);
			button = undefined;
			ctx.removeControl(control);
		};
	});

	$effect(() => {
		const el = button;
		if (!el) return;
		el.title = title;
		el.setAttribute('aria-label', title);
		el.setAttribute('aria-pressed', String(active));
		el.classList.toggle('is-active', active);
	});
</script>

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
