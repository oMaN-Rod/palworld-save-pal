<script lang="ts">
	import type { IControl } from 'maplibre-gl';
	import { untrack } from 'svelte';
	import { getMapContext, type ControlPosition } from '$components/maplibre';

	let {
		active = false,
		title,
		buttonClass,
		position = 'top-right',
		onchange
	}: {
		active?: boolean;
		title: string;
		/** Selector the caller styles its icon through, e.g. `maplibregl-ctrl-3d`. */
		buttonClass: string;
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
		el.className = untrack(() => buttonClass);

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
