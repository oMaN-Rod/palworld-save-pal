<script lang="ts">
	import { onDestroy } from 'svelte';
	import type maplibregl from 'maplibre-gl';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import * as m from '$i18n/messages';
	import MapComponent from '../Map.svelte';
	import { mapOptionsState } from '../state/mapOptions.svelte';
	import { DEFAULT_MAP_AREA } from '../geo/utils';
	import {
		clampToViewport,
		cornerOrigin,
		nearestCorner,
		PIP_PANEL_DEFAULT_HEIGHT,
		PIP_PANEL_DEFAULT_WIDTH,
		PIP_PANEL_MARGIN,
		PIP_PANEL_MIN_HEIGHT,
		PIP_PANEL_MIN_WIDTH,
		type PipRect
	} from './pipSnap';
	import { closeDocPipWindow, isDocumentPipSupported, openDocPipWindow } from './pipWindow';

	let { onClose }: { onClose: () => void } = $props();

	const mapOptions = $derived(mapOptionsState.current);

	let rect = $state<PipRect>({
		x: PIP_PANEL_MARGIN,
		y: PIP_PANEL_MARGIN,
		width: PIP_PANEL_DEFAULT_WIDTH,
		height: PIP_PANEL_DEFAULT_HEIGHT
	});

	let mode = $state<'inline' | 'docpip'>('inline');
	let pipMap: maplibregl.Map | undefined = $state();
	let contentEl: HTMLDivElement;
	let pipWindow: Window | null = null;
	let dragOrigin: { pointerX: number; pointerY: number; rectX: number; rectY: number } | null =
		null;

	function viewportSize() {
		return { width: window.innerWidth, height: window.innerHeight };
	}

	$effect(() => {
		function handleViewportResize() {
			if (mode !== 'inline') return;
			const { width, height } = viewportSize();
			rect = clampToViewport(rect, width, height, PIP_PANEL_MIN_WIDTH, PIP_PANEL_MIN_HEIGHT);
		}
		handleViewportResize();
		window.addEventListener('resize', handleViewportResize);
		return () => window.removeEventListener('resize', handleViewportResize);
	});

	function handleDragPointerDown(event: PointerEvent) {
		if (mode !== 'inline') return;
		if ((event.target as HTMLElement | null)?.closest('button')) return;
		dragOrigin = { pointerX: event.clientX, pointerY: event.clientY, rectX: rect.x, rectY: rect.y };
		(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
	}

	function handleDragPointerMove(event: PointerEvent) {
		if (!dragOrigin) return;
		const { width, height } = viewportSize();
		rect = clampToViewport(
			{
				x: dragOrigin.rectX + (event.clientX - dragOrigin.pointerX),
				y: dragOrigin.rectY + (event.clientY - dragOrigin.pointerY),
				width: rect.width,
				height: rect.height
			},
			width,
			height,
			PIP_PANEL_MIN_WIDTH,
			PIP_PANEL_MIN_HEIGHT
		);
	}

	function handleDragPointerUp() {
		if (!dragOrigin) return;
		dragOrigin = null;
		const { width, height } = viewportSize();
		const corner = nearestCorner(rect, width, height);
		rect = { ...rect, ...cornerOrigin(corner, rect.width, rect.height, width, height) };
	}

	async function popOut() {
		if (mode !== 'inline' || !isDocumentPipSupported()) return;
		try {
			pipWindow = await openDocPipWindow(contentEl, {
				width: rect.width,
				height: rect.height,
				onDismiss: () => onClose(),
				onResize: () => pipMap?.resize()
			});
			mode = 'docpip';
			requestAnimationFrame(() => pipMap?.resize());
		} catch (e) {
			console.warn('[pip] Document Picture-in-Picture request was refused', e);
		}
	}

	function handleCloseClick() {
		if (mode === 'docpip' && pipWindow) closeDocPipWindow(pipWindow);
		else onClose();
	}

	onDestroy(() => {
		if (mode === 'docpip' && pipWindow) closeDocPipWindow(pipWindow);
	});
</script>

<div
	class="pip-shell"
	class:pip-shell-inline={mode === 'inline'}
	style:left={mode === 'inline' ? `${rect.x}px` : undefined}
	style:top={mode === 'inline' ? `${rect.y}px` : undefined}
	style:width={mode === 'inline' ? `${rect.width}px` : undefined}
	style:height={mode === 'inline' ? `${rect.height}px` : undefined}
>
	<div bind:this={contentEl} class="pip-content" class:pip-docpip={mode === 'docpip'}>
		<div
			class="pip-header"
			onpointerdown={handleDragPointerDown}
			onpointermove={handleDragPointerMove}
			onpointerup={handleDragPointerUp}
			onpointercancel={() => (dragOrigin = null)}
		>
			<Icon icon="tabler:grip-horizontal" size={14} class="pip-grip" />
			<span class="pip-title">{m.pip_button()}</span>
			<div class="pip-actions">
				{#if isDocumentPipSupported()}
					<button
						type="button"
						class="pip-icon-btn"
						title={m.pip_pop_out()}
						aria-label={m.pip_pop_out()}
						onclick={popOut}
					>
						<Icon icon="tabler:external-link" size={14} />
					</button>
				{/if}
				<button
					type="button"
					class="pip-icon-btn"
					title={m.close()}
					aria-label={m.close()}
					onclick={handleCloseClick}
				>
					<Icon icon="tabler:x" size={14} />
				</button>
			</div>
		</div>
		<div class="pip-body">
			<MapComponent
				bind:map={pipMap}
				pip
				showLiveActors
				area={mapOptions.area ?? DEFAULT_MAP_AREA}
				showOrigin={mapOptions.showOrigin}
				showPlayers={mapOptions.showPlayers}
				showBases={mapOptions.showBases}
				showFastTravel={mapOptions.showFastTravel}
				showWatchtower={mapOptions.showWatchtower}
				showRelics={mapOptions.showRelics}
				hideCollectedRelics={mapOptions.hideCollectedRelics}
				hideUnlockedFastTravel={mapOptions.hideUnlockedFastTravel}
				relicTypes={mapOptions.relicTypes ?? {}}
				showDungeons={mapOptions.showDungeons}
				showBosses={mapOptions.showBosses}
				showAlphaPals={mapOptions.showAlphaPals}
				showPredatorPals={mapOptions.showPredatorPals}
				showBounty={mapOptions.showBounty}
				mapLayerVisibility={mapOptions.mapLayerVisibility ?? {}}
				showLabels={mapOptions.showLabels}
				show3d={mapOptions.enable3d}
				palSize={mapOptions.palSize}
				palAutoFollow={mapOptions.palAutoFollow}
				palHeight={mapOptions.palHeight}
				mapOpacity={mapOptions.mapOpacity}
				fastTravelSize={mapOptions.fastTravelSize}
				watchtowerSize={mapOptions.watchtowerSize}
				relicSize={mapOptions.relicSize}
				structureTypes={mapOptions.structureTypes ?? {}}
				renderMode={mapOptions.structureRenderMode ?? 'detailed'}
				structureTextured={mapOptions.structureTextured}
			/>
		</div>
	</div>
</div>

<style>
	.pip-shell-inline {
		position: fixed;
		z-index: 2000;
		min-width: 240px;
		min-height: 160px;
	}

	.pip-content {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		background: color-mix(in srgb, var(--color-surface-900) 95%, transparent);
		border: 1px solid var(--color-surface-700, #333);
		border-radius: 10px;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
		overflow: hidden;
	}

	.pip-docpip {
		border: none;
		border-radius: 0;
		box-shadow: none;
	}

	.pip-docpip .pip-header {
		display: none;
	}

	.pip-header {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 4px 6px;
		cursor: grab;
		touch-action: none;
		color: white;
		background: color-mix(in srgb, var(--color-surface-800) 90%, transparent);
	}

	.pip-title {
		flex: 1;
		font-size: 12px;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}

	.pip-actions {
		display: flex;
		align-items: center;
		gap: 2px;
	}

	.pip-icon-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 22px;
		height: 22px;
		color: white;
		border-radius: 6px;
	}

	.pip-icon-btn:hover {
		background: color-mix(in srgb, white 15%, transparent);
	}

	.pip-body {
		position: relative;
		flex: 1;
		min-height: 0;
	}
</style>
