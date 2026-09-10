<script lang="ts">
	import type { TreeNode, LabResearch } from '$types';
	import { Tooltip } from '$components/ui';
	import { ASSET_DATA_PATH, staticIcons } from '$types/icons';
	import { assetLoader } from '$utils';
	import { cn } from '$theme';
	import ResearchNode from './ResearchNode.svelte';
	import * as m from '$i18n/messages';

	let {
		node,
		selectedNode = $bindable(),
		unlockResearch = undefined,
		selectNode,
		nodeElements = $bindable(),
		showProgress = false
	} = $props<{
		node: TreeNode;
		selectedNode?: TreeNode | null;
		unlockResearch?: (node: TreeNode) => void;
		selectNode: (node: TreeNode) => void;
		nodeElements?: { [key: string]: HTMLElement };
		showProgress?: boolean;
	}>();

	let isHovered = $state(false);
	const isSelected = $derived(selectedNode?.id === node.id);

	function getNodeIcon(research: LabResearch): string {
		if (research.details.icon) {
			return (
				assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${research.details.icon}.webp`) ||
				staticIcons.unknownIcon
			);
		}
		return staticIcons.unknownIcon;
	}

	function handleClick() {
		if (unlockResearch && !node.isCompleted && node.isUnlocked) {
			unlockResearch(node);
		}
		selectNode(node);
	}

	const backgroundImageUrl = $derived.by(() => {
		const isEssential = node.research.details.is_essential;
		const essentialBit = isEssential ? '1' : '0';
		const showSelectedHighlight = isSelected || isHovered;

		let statePart: string;
		if (showSelectedHighlight) {
			statePart = 'selected';
		} else if (node.isCompleted) {
			statePart = 'on';
		} else {
			statePart = 'off';
		}

		const fileName = `t_prt_research_iconbase_${essentialBit}_${statePart}.webp`;
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${fileName}`);
	});

	const baseButtonClass =
		'flex h-20 w-20 items-center justify-center transition-all relative bg-cover bg-center 2xl:h-24 2xl:w-24 focus:outline-none';

	const progress = $derived(
		node.totalWorkAmount > 0 ? Math.min(1, node.workAmount / node.totalWorkAmount) : 0
	);

	const nodeOpacity = $derived(
		!showProgress
			? !node.isUnlocked && !node.isCompleted
				? 0.5
				: 1
			: node.isCompleted
				? 1
				: 0.34 + 0.56 * progress
	);

	const dynamicButtonClass = $derived(
		cn(!node.isUnlocked && !node.isCompleted && 'cursor-not-allowed')
	);
</script>

<div class="flex flex-col items-center">
	<div bind:this={nodeElements[node.id]} class="relative z-10">
		<Tooltip position="top" popupClass="bg-surface-800 p-2 text-xs">
			<button
				class={cn(baseButtonClass, dynamicButtonClass)}
				style:background-image={backgroundImageUrl ? `url('${backgroundImageUrl}')` : 'none'}
				style:opacity={nodeOpacity}
				onclick={handleClick}
				disabled={!!unlockResearch && !node.isUnlocked && !node.isCompleted}
				onmouseenter={() => {
					isHovered = true;
					selectNode(node);
				}}
				onmouseleave={() => (isHovered = false)}
				onfocus={() => selectNode(node)}
				aria-label={node.research.localized_name}
			>
				{#if showProgress && !node.isCompleted && progress > 0}
					<svg class="pointer-events-none absolute inset-0" viewBox="0 0 68 68" aria-hidden="true">
						<circle cx="34" cy="34" r="30" fill="none" stroke="var(--color-surface-700)" stroke-width="3" />
						<circle
							cx="34"
							cy="34"
							r="30"
							fill="none"
							stroke="var(--color-primary-500)"
							stroke-width="3"
							stroke-linecap="round"
							stroke-dasharray={2 * Math.PI * 30}
							stroke-dashoffset={2 * Math.PI * 30 * (1 - progress)}
							transform="rotate(-90 34 34)"
						/>
					</svg>
				{/if}
				<img
					src={getNodeIcon(node.research)}
					alt=""
					class="z-10 h-8 w-8 2xl:h-12 2xl:w-12"
					draggable="false"
				/>
			</button>
			{#snippet popup()}
				<div class="text-center">
					{node.research.localized_name}
					{#if showProgress && !node.isCompleted && progress > 0}
						<br /><span class="text-primary-400 text-xs tabular-nums"
							>{Math.round(progress * 100)}%</span
						>
					{/if}
					{#if unlockResearch && !node.isCompleted && node.isUnlocked}
						<br /><span class="text-warning-400 text-xs">({m.click_to_complete()})</span>
					{/if}
					{#if node.isCompleted}
						<br /><span class="text-success-400 text-xs">({m.completed()})</span>
					{/if}
					{#if !node.isUnlocked}
						<br /><span class="text-error-400 text-xs">({m.locked()})</span>
					{/if}
				</div>
			{/snippet}
		</Tooltip>
	</div>

	{#if node.children.length > 0}
		<div class="mt-6 flex justify-center space-x-6">
			{#each node.children as childNode (childNode.id)}
				<ResearchNode
					node={childNode}
					bind:selectedNode
					{unlockResearch}
					{selectNode}
					bind:nodeElements
					{showProgress}
				/>
			{/each}
		</div>
	{/if}
</div>
