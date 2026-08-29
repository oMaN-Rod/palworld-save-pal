<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { onMount, onDestroy, untrack } from 'svelte';
	import * as m from '$i18n/messages';
	import { getToastState, theme } from '$states';
	import type { BreedablePal, Chain } from '$lib/breeding/types';
	import { DendrogramEngine } from '$lib/breeding/dendrogram/DendrogramEngine';
	import type { LayoutMode } from '$lib/breeding/dendrogram/layouts';
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
		layoutMode = 'dendrogram',
		onselect
	}: {
		chain?: Chain;
		treeNode?: TreeNode;
		palMap: Map<string, BreedablePal>;
		passiveName?: (asset: string) => string;
		height?: number;
		fullHeight?: boolean;
		layoutMode?: LayoutMode;
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
		// Keeps receiving moves outside the SVG during a drag; also makes touch panning work (mouse-only events never fire on touch).
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

	const tree = $derived(treeNode ?? chainToTree(chain!, palMap));

	// Deliberately separate from the render effect: `onselect` is an inline arrow at the call site, so its
	// identity changes on every parent render. Folding it into the render effect made clicking a node
	// re-render the tree — clearing the selection it had just set and snapping zoom/pan back to fit.
	$effect(() => {
		if (!engine) return;
		engine.passiveName = passiveName;
		engine.matchedPassives = matchedPassives;
		engine.callbacks.onSelect = (node) => onselect?.(node);
	});

	$effect(() => {
		const nextTree = tree;
		const nextMode = layoutMode;
		void theme.current; // read to track it as a dependency; the render call below doesn't use it directly
		if (!engine) return;
		untrack(() => {
			engine.layoutMode = nextMode;
			engine.render(nextTree);
			requestAnimationFrame(() => engine.fit());
		});
	});

	$effect(() => {
		void fullHeight;
		void height;
		if (engine) requestAnimationFrame(() => engine.fit());
	});

	onMount(() => {
		engine = new DendrogramEngine(svgEl);
		engine.passiveName = passiveName;
		engine.matchedPassives = matchedPassives;
		engine.callbacks.onSelect = (node) => onselect?.(node);
		engine.layoutMode = layoutMode;
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
	class="bg-surface-950/80 border-surface-700/40 relative w-full overflow-hidden border {fullHeight
		? 'h-full'
		: 'rounded-md'}"
	style:height={fullHeight ? null : `${height}px`}
>
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<svg
		bind:this={svgEl}
		class="block h-full w-full"
		style="touch-action: none;"
		role="application"
		tabindex="0"
		aria-label="Breeding chain tree for {treeNode?.tribe ?? chain?.target ?? 'unknown'}"
		onpointerdown={handlePointerDown}
		onpointermove={handlePointerMove}
		onpointerup={handlePointerUp}
		onpointerleave={handlePointerLeave}
	></svg>

	<div class="absolute top-2 right-2 z-10 flex flex-col gap-1">
		<button
			class="btn btn-secondary rounded-sm text-surface-200 hover:text-surface-50 p-1.5"
			title={m.breeding_zoom_in()}
			onclick={() => engine?.zoomBy(1.25)}
		>
			<Icon icon="tabler:plus" size={14} />
		</button>
		<button
			class="btn btn-secondary rounded-sm text-surface-200 hover:text-surface-50 p-1.5"
			title={m.breeding_zoom_out()}
			onclick={() => engine?.zoomBy(0.8)}
		>
			<Icon icon="tabler:minus" size={14} />
		</button>
		<button
			class="btn btn-secondary rounded-sm text-surface-200 hover:text-surface-50 p-1.5"
			title={m.breeding_fit_view()}
			onclick={() => engine?.fit()}
		>
			<Icon icon="tabler:maximize" size={14} />
		</button>
		<div class="bg-surface-700/40 my-0.5 h-px"></div>
		<button
			class="btn btn-secondary rounded-sm text-surface-200 hover:text-surface-50 p-1.5 disabled:cursor-not-allowed disabled:opacity-40"
			title={m.breeding_export_png()}
			disabled={exporting}
			onclick={() => handleExport('download')}
		>
			<Icon icon="tabler:download" size={14} class={exporting ? 'animate-pulse' : ''} />
		</button>
		<button
			class="btn btn-secondary rounded-sm text-surface-200 hover:text-surface-50 p-1.5 disabled:cursor-not-allowed disabled:opacity-40"
			title={copied ? m.breeding_png_copied() : m.breeding_copy_png()}
			disabled={exporting}
			onclick={() => handleExport('copy')}
		>
			{#if copied}<Icon icon="tabler:check" size={14} class="text-success-400" />{:else}<Icon icon="tabler:copy" size={14} />{/if}
		</button>
	</div>

	<ChainTooltip node={hoveredNode} x={tooltipX} y={tooltipY} {passiveName} />
</div>
