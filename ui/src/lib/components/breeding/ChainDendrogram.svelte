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
	import type { BreedablePal, Chain } from '$lib/breeding/types';
	import { DendrogramEngine } from '$lib/breeding/dendrogram/DendrogramEngine';
	import { chainToTree } from '$lib/breeding/dendrogram/treeBuilder';
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

	let svgEl: SVGSVGElement;
	let containerEl: HTMLDivElement;
	let engine: DendrogramEngine;
	let resizeObserver: ResizeObserver | null = null;

	let hoveredNode = $state<TreeNode | null>(null);
	let tooltipX = $state(0);
	let tooltipY = $state(0);

	let mouseDownX = 0;
	let mouseDownY = 0;
	let hasMoved = false;
	let mouseDownButton = 0;

	const matchedPassives = $derived(new Set(chain?.matched_passives ?? []));

	function getSvgPos(e: MouseEvent): [number, number] {
		const rect = svgEl.getBoundingClientRect();
		return [e.clientX - rect.left, e.clientY - rect.top];
	}

	function handleMouseDown(e: MouseEvent) {
		const [sx, sy] = getSvgPos(e);
		mouseDownX = sx;
		mouseDownY = sy;
		mouseDownButton = e.button;
		hasMoved = false;
	}

	function handleMouseMove(e: MouseEvent) {
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

	function handleMouseUp(e: MouseEvent) {
		if (mouseDownButton !== 0 || hasMoved) return;
		const [sx, sy] = getSvgPos(e);
		const hit = engine.hitTestNode(sx, sy);
		engine.setSelected(hit?.id ?? null);
		onselect?.(hit ?? null);
	}

	function handleMouseLeave() {
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
		onmousedown={handleMouseDown}
		onmousemove={handleMouseMove}
		onmouseup={handleMouseUp}
		onmouseleave={handleMouseLeave}
	></svg>

	<div class="absolute top-2 right-2 flex flex-col gap-1 z-10">
		<button class="btn btn-secondary p-1.5 rounded-4 text-surface-200 hover:text-surface-50" title="Zoom in" onclick={() => engine?.zoomBy(1.25)}>
			<Plus size={14} />
		</button>
		<button class="btn btn-secondary p-1.5 rounded-4 text-surface-200 hover:text-surface-50" title="Zoom out" onclick={() => engine?.zoomBy(0.8)}>
			<Minus size={14} />
		</button>
		<button class="btn btn-secondary p-1.5 rounded-4 text-surface-200 hover:text-surface-50" title="Fit view" onclick={() => engine?.fit()}>
			<Maximize2 size={14} />
		</button>
	</div>

	<ChainTooltip node={hoveredNode} x={tooltipX} y={tooltipY} {passiveName} />
</div>
