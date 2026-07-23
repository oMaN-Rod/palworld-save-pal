<script lang="ts">
	import maplibregl from 'maplibre-gl';
	import { untrack } from 'svelte';
	import { getMapContext } from '../../contexts.svelte.js';
	import type { PopupProps } from './types.js';

	let {
		lnglat = $bindable(),
		open = $bindable(false),
		offset,
		anchor,
		maxWidth = '240px',
		closeButton = true,
		closeOnClick = true,
		closeOnMove = false,
		focusAfterOpen,
		className,
		subpixelPositioning,
		locationOccludedOpacity,
		padding,
		onopen,
		onclose,
		children
	}: PopupProps = $props();

	const ctx = getMapContext();

	let popup: maplibregl.Popup | undefined;
	let contentEl = $state<HTMLDivElement>();

	$effect(() => {
		const map = ctx.map;
		if (!map || !contentEl) return;

		const opts: maplibregl.PopupOptions = untrack(() => {
			const o: maplibregl.PopupOptions = {
				maxWidth,
				closeButton,
				closeOnClick,
				closeOnMove
			};
			if (offset != null) o.offset = offset;
			if (anchor) o.anchor = anchor;
			if (focusAfterOpen != null) o.focusAfterOpen = focusAfterOpen;
			if (className) o.className = className;
			if (subpixelPositioning != null) o.subpixelPositioning = subpixelPositioning;
			if (locationOccludedOpacity != null) o.locationOccludedOpacity = locationOccludedOpacity;
			if (padding) o.padding = padding;
			return o;
		});

		popup = new maplibregl.Popup(opts);
		popup.setDOMContent(contentEl);

		popup.on('open', () => {
			open = true;
			onopen?.();
		});

		popup.on('close', () => {
			open = false;
			onclose?.();
		});

		if (untrack(() => open && lnglat)) {
			popup.setLngLat(untrack(() => lnglat!)).addTo(map);
		}

		return () => {
			popup?.remove();
			popup = undefined;
		};
	});

	// React to open/lnglat changes
	$effect(() => {
		if (!popup || !ctx.map) return;

		if (open && lnglat) {
			if (!popup.isOpen()) {
				popup.setLngLat(lnglat).addTo(ctx.map);
			}
		} else if (!open) {
			if (popup.isOpen()) {
				popup.remove();
			}
		}
	});

	// React to lnglat changes while open
	$effect(() => {
		if (!popup || !lnglat) return;
		if (popup.isOpen()) {
			popup.setLngLat(lnglat);
		}
	});
</script>

<div bind:this={contentEl} style="display:contents;">
	{@render children?.()}
</div>
