<script lang="ts">
	/**
	 * ChainDendrogram — thin Svelte wrapper around DendrogramEngine for ONE
	 * breeding chain. Handles DOM events, ResizeObserver, lifecycle, and the
	 * zoom/fit/reset toolbar.
	 */
	import { onMount, onDestroy } from 'svelte';
	import Plus from '@lucide/svelte/icons/plus';
	import Minus from '@lucide/svelte/icons/minus';
	import Maximize2 from '@lucide/svelte/icons/maximize-2';
	import Download from '@lucide/svelte/icons/download';
	import Copy from '@lucide/svelte/icons/copy';
	import Check from '@lucide/svelte/icons/check';
	import * as m from '$i18n/messages';
	import { getToastState } from '$states';
	import type { BreedablePal, Chain } from '$lib/breeding/types';
	import { DendrogramEngine } from '$lib/breeding/dendrogram/DendrogramEngine';
	import { chainToTree } from '$lib/breeding/dendrogram/treeBuilder';
	import {
		copyPngToClipboard,
		downloadPng,
		exportTreeToPng,
		slugify
	} from '$lib/breeding/dendrogram/exportPng';
	import type { TreeNode } from '$lib/breeding/dendrogram/types';
	import ChainTooltip from './ChainTooltip.svelte';

	let {
		chain,
		treeNode,
		palMap,
		passiveName = (asset: string) => asset,
		height = 420,
		fullHeight = false,
		onselect
	}: {
		chain?: Chain;
		treeNode?: TreeNode;
		palMap: Map<string, BreedablePal>;
		passiveName?: (asset: string) => string;
		height?: number;
		fullHeight?: boolean;
		onselect?: (node: TreeNode | null) => void;
	} = $props();

	const toast = getToastState();

	let svgEl: SVGSVGElement;
	let containerEl: HTMLDivElement;
	let engine: DendrogramEngine;
	let resizeObserver: ResizeObserver | null = null;

	let hoveredNode = $state<TreeNode | null>(null);
	let tooltipX = $state(0);
	let tooltipY = $state(0);
	let exporting = $state(false);
	let copied = $state(false);

	const exportName = $derived(slugify(treeNode?.tribe ?? chain?.target ?? 'direct'));

	async function handleExport(kind: 'download' | 'copy') {
		if (!svgEl || exporting) return;
		exporting = true;
		try {
			const blob = await exportTreeToPng(svgEl);
			if (kind === 'download') {
				downloadPng(blob, `${exportName}-dendrogram.png`);
				toast.add(m.breeding_png_downloaded(), m.success(), 'success');
			} else {
				const ok = await copyPngToClipboard(blob);
				if (ok) {
					copied = true;
					setTimeout(() => (copied = false), 2000);
					toast.add(m.breeding_png_copied(), m.success(), 'success');
				} else {
					toast.add(m.breeding_png_copy_failed(), m.error(), 'error');
				}
			}
		} catch (e) {
			toast.add(e instanceof Error ? e.message : String(e), m.error(), 'error');
		} finally {
			exporting = false;
		}
	}

	let mouseDownX = 0;
	let mouseDownY = 0;
	let hasMoved = false;
	let mouseDownButton = 0;

	const matchedPassives = $derived(new Set(chain?.matched_passives ?? []));

	function getSvgPos(e: MouseEvent): [number, number] {
		const rect = svgEl.getBoundingClientRect();
		return [e.clientX - rect.left, e.clientY - rect.top];
	}

	function handlePointerDown(e: PointerEvent) {
		const [sx, sy] = getSvgPos(e);
		mouseDownX = sx;
		mouseDownY = sy;
		mouseDownButton = e.button;
		hasMoved = false;
		// Keep receiving moves outside the SVG during a drag (also makes touch
		// panning work — mouse-only events never fire on touch).
		svgEl.setPointerCapture?.(e.pointerId);
	}

	function handlePointerMove(e: PointerEvent) {
		const [sx, sy] = getSvgPos(e);
		if (e.buttons > 0) {
			const dx = sx - mouseDownX;
			const dy = sy - mouseDownY;
			if (Math.abs(dx) > 3 || Math.abs(dy) > 3) hasMoved = true;
		}
		const hit = engine.hitTestNode(sx, sy);
		const prevId = engine.hoveredId;
		if (hit) {
			engine.setHovered(hit.id);
			if (hit.id !== prevId) hoveredNode = hit;
			tooltipX = e.clientX;
			tooltipY = e.clientY;
			svgEl.style.cursor = 'pointer';
		} else {
			if (prevId !== null) engine.setHovered(null);
			hoveredNode = null;
			svgEl.style.cursor = 'grab';
		}
	}

	function handlePointerUp(e: PointerEvent) {
		if (mouseDownButton !== 0 || hasMoved) return;
		const [sx, sy] = getSvgPos(e);
		const hit = engine.hitTestNode(sx, sy);
		engine.setSelected(hit?.id ?? null);
		onselect?.(hit ?? null);
	}

	function handlePointerLeave() {
		engine.setHovered(null);
		hoveredNode = null;
		svgEl.style.cursor = 'default';
	}

	$effect(() => {
		void chain;
		void treeNode;
		void palMap;
		void matchedPassives;
		void onselect;
		if (!engine) return;
		const tree = treeNode ?? chainToTree(chain!, palMap);
		engine.passiveName = passiveName;
		engine.matchedPassives = matchedPassives;
		engine.callbacks.onSelect = (node) => onselect?.(node);
		engine.render(tree);
		requestAnimationFrame(() => engine.fit());
	});

	$effect(() => {
		void fullHeight;
		if (engine) requestAnimationFrame(() => engine.fit());
	});

	onMount(() => {
		engine = new DendrogramEngine(svgEl);
		engine.passiveName = passiveName;
		engine.matchedPassives = matchedPassives;
		engine.callbacks.onSelect = (node) => onselect?.(node);
		const tree = treeNode ?? chainToTree(chain!, palMap);
		engine.render(tree);

		resizeObserver = new ResizeObserver(() => {
			requestAnimationFrame(() => engine.fit());
		});
		resizeObserver.observe(containerEl);

		requestAnimationFrame(() => engine.fit());
		svgEl.style.cursor = 'grab';
	});

	onDestroy(() => {
		resizeObserver?.disconnect();
		engine?.destroy();
	});
</script>

<div
	bind:this={containerEl}
	class="relative w-full overflow-hidden bg-surface-950/80 border border-surface-700/40 {fullHeight ? 'h-full' : 'rounded-6'}"
	style={fullHeight ? '' : 'height: {height}px;'}
>
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<svg
		bind:this={svgEl}
		class="block w-full h-full"
		style="touch-action: none;"
		role="application"
		tabindex="0"
		aria-label="Breeding chain tree for {treeNode?.tribe ?? chain?.target ?? 'unknown'}"
onpointerdown={handlePointerDown}
			onpointermove={handlePointerMove}
			onpointerup={handlePointerUp}
			onpointerleave={handlePointerLeave}
	></svg>

	<div class="absolute top-2 right-2 flex flex-col gap-1 z-10">
		<button class="btn btn-secondary p-1.5 rounded-4 text-surface-200 hover:text-surface-50" title={m.breeding_zoom_in()} onclick={() => engine?.zoomBy(1.25)}>
			<Plus size={14} />
		</button>
		<button class="btn btn-secondary p-1.5 rounded-4 text-surface-200 hover:text-surface-50" title={m.breeding_zoom_out()} onclick={() => engine?.zoomBy(0.8)}>
			<Minus size={14} />
		</button>
		<button class="btn btn-secondary p-1.5 rounded-4 text-surface-200 hover:text-surface-50" title={m.breeding_fit_view()} onclick={() => engine?.fit()}>
			<Maximize2 size={14} />
		</button>
		<div class="my-0.5 h-px bg-surface-700/40"></div>
		<button
			class="btn btn-secondary p-1.5 rounded-4 text-surface-200 hover:text-surface-50 disabled:opacity-40 disabled:cursor-not-allowed"
			title={m.breeding_export_png()}
			disabled={exporting}
			onclick={() => handleExport('download')}
		>
			<Download size={14} class={exporting ? 'animate-pulse' : ''} />
		</button>
		<button
			class="btn btn-secondary p-1.5 rounded-4 text-surface-200 hover:text-surface-50 disabled:opacity-40 disabled:cursor-not-allowed"
			title={copied ? m.breeding_png_copied() : m.breeding_copy_png()}
			disabled={exporting}
			onclick={() => handleExport('copy')}
		>
			{#if copied}<Check size={14} class="text-emerald-400" />{:else}<Copy size={14} />{/if}
		</button>
	</div>

	<ChainTooltip node={hoveredNode} x={tooltipX} y={tooltipY} {passiveName} />
</div>
