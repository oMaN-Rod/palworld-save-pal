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
		unlockResearch,
		selectNode,
		nodeElements = $bindable()
	} = $props<{
		node: TreeNode;
		selectedNode?: TreeNode | null;
		unlockResearch: (node: TreeNode) => void;
		selectNode: (node: TreeNode) => void;
		nodeElements?: { [key: string]: HTMLElement };
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
		if (!node.isCompleted && node.isUnlocked) {
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

	const dynamicButtonClass = $derived(
		cn(!node.isUnlocked && !node.isCompleted && 'cursor-not-allowed opacity-50')
	);
</script>

<div class="flex flex-col items-center">
	<div bind:this={nodeElements[node.id]} class="relative z-10">
		<Tooltip position="top" popupClass="bg-surface-800 p-2 text-xs">
			<button
				class={cn(baseButtonClass, dynamicButtonClass)}
				style:background-image={backgroundImageUrl ? `url('${backgroundImageUrl}')` : 'none'}
				onclick={handleClick}
				disabled={!node.isUnlocked && !node.isCompleted}
				onmouseenter={() => {
					isHovered = true;
					selectNode(node);
				}}
				onmouseleave={() => (isHovered = false)}
				onfocus={() => selectNode(node)}
				aria-label={node.research.localized_name}
			>
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
					{#if !node.isCompleted && node.isUnlocked}
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
				<!-- Direct recursive call using the component's name -->
				<ResearchNode
					node={childNode}
					bind:selectedNode
					{unlockResearch}
					{selectNode}
					bind:nodeElements
				/>
			{/each}
		</div>
	{/if}
</div>
