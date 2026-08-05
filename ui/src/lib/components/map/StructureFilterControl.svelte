<script lang="ts">
	import type { IControl } from 'maplibre-gl';
	import { untrack } from 'svelte';
	import { getMapContext, type ControlPosition } from '$components/maplibre';
	import { Switch } from '@skeletonlabs/skeleton-svelte';
	import { Eye, EyeClosed } from '@lucide/svelte';

	let {
		types,
		enabled,
		open,
		onToggleOpen,
		ontoggle,
		position = 'top-right',
		title
	}: {
		types: string[];
		enabled: Record<string, boolean>;
		open: boolean;
		onToggleOpen: () => void;
		ontoggle: (type: string) => void;
		position?: ControlPosition;
		title: string;
	} = $props();

	const ctx = getMapContext();

	// Read through a holder so a new inline callback identity cannot re-create
	// the control on every parent render.
	const latest: { onToggleOpen?: () => void } = { onToggleOpen: untrack(() => onToggleOpen) };
	$effect(() => {
		latest.onToggleOpen = onToggleOpen;
	});

	let button = $state<HTMLButtonElement>();
	let panel = $state<HTMLDivElement>();

	$effect(() => {
		const map = ctx.map;
		if (!map) return;

		// Mirrors Toggle3dControl's ctrl-group/button/icon markup, plus a panel div
		// (rendered by the template below) appended as a second child so it can
		// open beneath the button.
		const container = document.createElement('div');
		container.className = 'maplibregl-ctrl maplibregl-ctrl-group structure-filter-ctrl';

		const el = document.createElement('button');
		el.type = 'button';
		el.className = 'maplibregl-ctrl-structure-filter';

		const icon = document.createElement('span');
		icon.className = 'maplibregl-ctrl-icon';
		icon.setAttribute('aria-hidden', 'true');
		el.appendChild(icon);
		container.appendChild(el);

		const handleClick = () => latest.onToggleOpen?.();
		el.addEventListener('click', handleClick);

		const panelEl = untrack(() => panel);
		if (panelEl) container.appendChild(panelEl);

		const control: IControl = {
			onAdd: () => container,
			onRemove: () => {
				// Detach the Svelte-owned panel first so Svelte's own teardown finds it
				// already parentless instead of racing to remove it from a container
				// that's about to be dropped.
				if (panelEl && panelEl.parentNode === container) {
					container.removeChild(panelEl);
				}
				container.remove();
			}
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
		el.setAttribute('aria-expanded', String(open));
		el.classList.toggle('is-active', open);
	});
</script>

<div bind:this={panel} class="structure-filter-panel" class:open>
	{#each types as type (type)}
		<Switch checked={enabled[type] !== false} onCheckedChange={() => ontoggle(type)} compact classes="h-6 w-6">
			<span>{type}</span>
			{#snippet inactiveChild()}<EyeClosed class="w-4 h-4" />{/snippet}
			{#snippet activeChild()}<Eye class="w-4 h-4" />{/snippet}
		</Switch>
	{/each}
</div>

<style>
	.structure-filter-panel {
		display: none;
		position: absolute;
		top: 100%;
		right: 0;
		margin-top: 4px;
		min-width: 160px;
		overflow-y: auto;
		padding: 6px;
		background: var(--svlibre-ctrl-bg, #fff);
		color: var(--svlibre-ctrl-color, #333);
		border-radius: 4px;
		box-shadow: var(--svlibre-ctrl-shadow, 0 0 0 2px rgba(0, 0, 0, 0.1));
	}

	.structure-filter-panel.open {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.structure-filter-row {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 3px 4px;
		border-radius: 3px;
		font-size: 12px;
		white-space: nowrap;
		cursor: pointer;
	}

	.structure-filter-row:hover {
		background: var(--svlibre-ctrl-bg-hover, rgba(0, 0, 0, 0.05));
	}

	:global(.structure-filter-ctrl) {
		position: relative;
	}

	:global(.maplibregl-ctrl button.maplibregl-ctrl-structure-filter .maplibregl-ctrl-icon) {
		background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='29' height='29' viewBox='-2.5 -2.5 29 29' fill='none' stroke='%23333' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpolygon points='22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3'/%3E%3C/svg%3E");
	}

	/* Matches maplibre's own convention for an inactive icon. Theme inversion is
	   handled centrally by the map wrapper, alongside the built-in controls. */
	:global(
		.maplibregl-ctrl button.maplibregl-ctrl-structure-filter:not(.is-active) .maplibregl-ctrl-icon
	) {
		opacity: 0.25;
	}
</style>
