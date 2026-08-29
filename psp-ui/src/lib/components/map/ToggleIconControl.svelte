<script lang="ts">
	import type { IControl } from 'maplibre-gl';
	import { untrack } from 'svelte';
	import { getMapContext, type ControlPosition } from '$components/maplibre';

	let {
		active = false,
		title,
		buttonClass,
		label,
		position = 'top-right',
		onchange
	}: {
		active?: boolean;
		title: string;
		buttonClass: string;
		label?: string;
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
	let iconEl = $state<HTMLSpanElement>();

	$effect(() => {
		const map = ctx.map;
		if (!map) return;

		const container = document.createElement('div');
		container.className = 'maplibregl-ctrl maplibregl-ctrl-group';

		const el = document.createElement('button');
		el.type = 'button';
		el.className = untrack(() => buttonClass);

		const icon = document.createElement('span');
		icon.className = 'maplibregl-ctrl-icon';
		icon.setAttribute('aria-hidden', 'true');
		const initialLabel = untrack(() => label);
		if (initialLabel !== undefined) icon.classList.add('maplibregl-ctrl-icon-text');
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
		iconEl = icon;

		return () => {
			el.removeEventListener('click', handleClick);
			button = undefined;
			iconEl = undefined;
			ctx.removeControl(control);
		};
	});

	$effect(() => {
		const el = iconEl;
		if (!el || label === undefined) return;
		el.textContent = label;
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
