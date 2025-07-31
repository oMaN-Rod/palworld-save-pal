<script lang="ts">
	import { getToastState } from '$states';
	import { labResearchData, itemsData } from '$lib/data';
	import { Card } from '$components/ui';
	import { type Guild, MessageType, type LabResearch, type TreeNode, EntryState } from '$types';
	import { ASSET_DATA_PATH, staticIcons } from '$types/icons';
	import { assetLoader, debounce, deepCopy } from '$utils';
	import { send } from '$lib/utils/websocketUtils';
	import ResearchNode from './ResearchNode.svelte';
	import { onMount } from 'svelte';
	let { guild = $bindable(), selectedCategory = $bindable('Handiwork') } = $props<{
		guild: Guild;
		selectedCategory: string;
	}>();

	const toast = getToastState();

	let selectedNode: TreeNode | null = $state(null);
	let researchTree: Record<string, TreeNode[]> = $state({});
	let categories: string[] = $state([]);
	let nodeElements: { [key: string]: HTMLElement } = $state({});
	let lineCoords: { [key: string]: { x1: number; y1: number; x2: number; y2: number } } = $state(
		{}
	);

	function getGuildResearchProgress(researchId: string): number {
		if (!guild || !guild.lab_research_data) return 0;
		const researchInfo = guild.lab_research_data.find(
			(r: { research_id: string }) => r.research_id === researchId
		);
		return researchInfo ? researchInfo.work_amount : 0;
	}

	function buildTree(allResearch: Record<string, LabResearch>, category: string): TreeNode[] {
		const categoryResearch = Object.values(allResearch).filter(
			(r) => r.details.category === category
		);
		const nodes: Record<string, TreeNode> = {};
		const rootNodes: TreeNode[] = [];

		categoryResearch.forEach((r) => {
			const workAmount = getGuildResearchProgress(r.id);
			const totalWorkAmount = r.details.work_amount || 0;
			const isCompleted = workAmount >= totalWorkAmount && totalWorkAmount > 0;
			nodes[r.id] = {
				id: r.id,
				research: r,
				children: [],
				isUnlocked: false,
				isCompleted: isCompleted,
				workAmount: workAmount,
				totalWorkAmount: totalWorkAmount
			};
		});

		const processNodeHierarchy = (node: TreeNode) => {
			const prerequisiteId = node.research.details.require_research_id;
			if (prerequisiteId && nodes[prerequisiteId]) {
				node.isUnlocked = nodes[prerequisiteId].isCompleted;
			} else {
				node.isUnlocked = true;
			}
			node.children.forEach(processNodeHierarchy);
		};

		categoryResearch.forEach((r) => {
			const node = nodes[r.id];
			const prerequisiteId = r.details.require_research_id;
			if (prerequisiteId && nodes[prerequisiteId]) {
				nodes[prerequisiteId].children.push(node);
			} else {
				rootNodes.push(node);
			}
		});

		rootNodes.forEach(processNodeHierarchy);

		Object.values(nodes).forEach((node) => node.children.sort((a, b) => a.id.localeCompare(b.id)));

		return rootNodes;
	}

	function getRequiredWork(researchId: string): number {
		return labResearchData.research[researchId]?.details.work_amount ?? 0;
	}

	function findNodeById(nodes: TreeNode[] | undefined, id: string): TreeNode | null {
		if (!nodes) return null;
		for (const node of nodes) {
			if (node.id === id) return node;
			const found = findNodeById(node.children, id);
			if (found) return found;
		}
		return null;
	}

	async function unlockAllForCategory(category: string) {
		if (!guild?.lab_research_data || !researchTree[category]) {
			return;
		}

		const categoryResearch = Object.values(labResearchData.research).filter(
			(r) => r.details.category === category
		);

		const updatedResearchList = deepCopy(guild.lab_research_data || []);
		const itemsToUpdateSet = new Set<string>();

		categoryResearch.forEach((research) => {
			itemsToUpdateSet.add(research.id);
		});

		itemsToUpdateSet.forEach((researchId) => {
			const requiredWork = getRequiredWork(researchId);
			let existingEntry = updatedResearchList.find(
				(r: { research_id: string }) => r.research_id === researchId
			);
			if (existingEntry) {
				existingEntry.work_amount = requiredWork;
			} else {
				updatedResearchList.push({ research_id: researchId, work_amount: requiredWork });
			}

			const updatedNode = findNodeById(researchTree[category], researchId);
			if (updatedNode) {
				updatedNode.workAmount = requiredWork;
				updatedNode.isCompleted = true;
				updatedNode.isUnlocked = true;
			}
		});

		const processNodeHierarchy = (node: TreeNode) => {
			const prerequisiteId = node.research.details.require_research_id;
			if (prerequisiteId && findNodeById(researchTree[category], prerequisiteId)) {
				const prereqNode = findNodeById(researchTree[category], prerequisiteId);
				node.isUnlocked = prereqNode ? prereqNode.isCompleted : true;
			} else {
				node.isUnlocked = true;
			}
			node.children.forEach(processNodeHierarchy);
		};

		researchTree[category].forEach(processNodeHierarchy);

		researchTree = { ...researchTree };
		guild.lab_research_data = updatedResearchList;
		guild.state = EntryState.MODIFIED;

		send(MessageType.UPDATE_LAB_RESEARCH, {
			guild_id: guild.id,
			research_updates: updatedResearchList
		});
	}

	export { unlockAllForCategory };

	async function unlockResearch(node: TreeNode) {
		if (node.isCompleted) return;
		if (!node.isUnlocked) {
			toast.add(
				`Unlock prerequisite first: ${labResearchData.research[node.research.details.require_research_id || '']?.localized_name || 'Unknown'}`,
				'Locked',
				'warning'
			);
			return;
		}

		const updatedResearchList = deepCopy(guild.lab_research_data || []);
		const prerequisitesToUnlock: string[] = [];

		const findAndUnlockPrerequisites = (researchId: string) => {
			const researchDef = labResearchData.research[researchId];
			if (!researchDef) return;
			const prerequisiteId = researchDef.details.require_research_id;
			if (prerequisiteId) {
				const prereqNode = findNodeById(researchTree[selectedCategory], prerequisiteId);
				if (prereqNode && !prereqNode.isCompleted) {
					prerequisitesToUnlock.push(prerequisiteId);
					findAndUnlockPrerequisites(prerequisiteId);
				}
			}
		};

		findAndUnlockPrerequisites(node.id);

		const itemsToUpdateSet = new Set([...prerequisitesToUnlock.reverse(), node.id]);

		itemsToUpdateSet.forEach((researchId) => {
			const requiredWork = getRequiredWork(researchId);
			let existingEntry = updatedResearchList.find(
				(r: { research_id: string }) => r.research_id === researchId
			);
			if (existingEntry) {
				existingEntry.work_amount = requiredWork;
			} else {
				updatedResearchList.push({ research_id: researchId, work_amount: requiredWork });
			}

			const updatedNode = findNodeById(researchTree[selectedCategory], researchId);
			if (updatedNode) {
				updatedNode.workAmount = requiredWork;
				updatedNode.isCompleted = true;
				updatedNode.isUnlocked = true;

				updatedNode.children.forEach((child) => {
					child.isUnlocked = true;

					child.isCompleted =
						child.workAmount >= child.totalWorkAmount && child.totalWorkAmount > 0;
				});
			}
		});

		researchTree = { ...researchTree };
		guild.lab_research_data = updatedResearchList;
		guild.state = EntryState.MODIFIED;

		send(MessageType.UPDATE_LAB_RESEARCH, {
			guild_id: guild.id,
			research_updates: updatedResearchList
		});
		selectedNode = node;
	}

	function selectNode(node: TreeNode) {
		selectedNode = node;
	}

	function getLineCoordinates(
		parentNode: HTMLElement,
		childNode: HTMLElement
	): { x1: number; y1: number; x2: number; y2: number } | null {
		if (!parentNode || !childNode) return null;

		const parentRect = parentNode.getBoundingClientRect();
		const childRect = childNode.getBoundingClientRect();
		const container = parentNode.closest('.research-tree-container');

		if (!container) return null;
		const containerRect = container.getBoundingClientRect();

		const x1 = parentRect.left + parentRect.width / 2 - containerRect.left + container.scrollLeft;
		const y1 = parentRect.bottom - containerRect.top + container.scrollTop;
		const x2 = childRect.left + childRect.width / 2 - containerRect.left + container.scrollLeft;
		const y2 = childRect.top - containerRect.top + container.scrollTop;

		return { x1, y1, x2, y2 };
	}

	function calculateAllLines() {
		if (
			!researchTree[selectedCategory] ||
			!nodeElements ||
			Object.keys(nodeElements).length === 0
		) {
			lineCoords = {};
			return;
		}

		const newCoords: typeof lineCoords = {};
		const queue: TreeNode[] = [...researchTree[selectedCategory]];

		while (queue.length > 0) {
			const node = queue.shift();
			if (!node) continue;

			const parentEl = nodeElements[node.id];
			if (!parentEl) continue;

			node.children.forEach((child) => {
				const childEl = nodeElements[child.id];
				if (childEl) {
					const coords = getLineCoordinates(parentEl, childEl);
					if (coords) {
						newCoords[`${node.id}->${child.id}`] = coords;
					}
				}
				queue.push(child);
			});
		}
		lineCoords = newCoords;
	}

	const debouncedCalculateLines = debounce(calculateAllLines, 150);

	$effect(() => {
		if (labResearchData.research && Object.keys(labResearchData.research).length > 0) {
			const uniqueCategories = [
				...new Set(Object.values(labResearchData.research).map((r) => r.details.category))
			].filter(Boolean);
			categories = uniqueCategories.sort() as string[];

			const newTree: Record<string, TreeNode[]> = {};
			uniqueCategories.forEach((cat) => {
				if (cat) {
					newTree[cat] = buildTree(labResearchData.research, cat);
				}
			});
			researchTree = newTree;
		}
	});

	$effect(() => {
		if (selectedCategory && researchTree[selectedCategory]) {
			setTimeout(() => {
				requestAnimationFrame(calculateAllLines);
			}, 50);
		}
	});

	$effect(() => {
		nodeElements;
		debouncedCalculateLines();
	});

	onMount(() => {
		window.addEventListener('resize', debouncedCalculateLines);
		const container = document.querySelector('.research-tree-container');
		if (container) {
			container.addEventListener('scroll', debouncedCalculateLines);
		}
		return () => {
			window.removeEventListener('resize', debouncedCalculateLines);
			if (container) {
				container.removeEventListener('scroll', debouncedCalculateLines);
			}
		};
	});
</script>

<div class="grid grid-cols-[1fr_400px] gap-4">
	<!-- Research Tree -->
	<div class="research-tree-container relative h-[calc(100vh-160px)] overflow-y-auto p-4">
		<svg class="pointer-events-none absolute left-0 top-0 z-0 w-full overflow-visible">
			{#each Object.entries(lineCoords) as [_key, coords]}
				<line
					x1={coords.x1}
					y1={coords.y1}
					x2={coords.x2}
					y2={coords.y2}
					stroke="var(--color-surface-600)"
					stroke-width="3"
				/>
			{/each}
		</svg>
		<div class="relative z-10 flex min-w-max flex-col items-center space-y-6 p-4">
			{#if researchTree[selectedCategory]}
				{#each researchTree[selectedCategory] as rootNode (rootNode.id)}
					<ResearchNode
						node={rootNode}
						{selectedNode}
						{unlockResearch}
						{selectNode}
						bind:nodeElements
					/>
				{/each}
			{:else}
				<p>Select a category.</p>
			{/if}
		</div>
	</div>

	<!-- Details Pane -->
	{#if selectedNode}
		<Card class="m-4 h-auto rounded-lg">
			{#if selectedNode}
				{@const research = selectedNode.research}
				<h5 class="h5 mb-2">{research.localized_name}</h5>

				{#if research.details.effect_type && research.details.effect_type !== 'None'}
					<div class="mb-4">
						<h6 class="h6 mb-1">Effect</h6>
						<p class="text-sm">
							{research.details.effect_type}: {research.details.effect_value &&
							research.details.effect_value > 0
								? '+'
								: ''}{research.details.effect_value ?? 0}%
							{#if research.details.effect_work_suitability && research.details.effect_work_suitability !== 'None'}
								for {research.details.effect_work_suitability}
							{/if}
							{#if research.details.effect_item_type && research.details.effect_item_type !== 'None'}
								for {research.details.effect_item_type}
							{/if}
						</p>
					</div>
				{/if}

				{#if research.details.materials && research.details.materials.length > 0}
					<h6 class="h6 mb-1">Research Cost</h6>
					<div class="space-y-1">
						{#each research.details.materials as material}
							{@const itemData = itemsData.getByKey(material.id)}
							<div class="flex items-center space-x-2 text-sm">
								{#if itemData}
									{@const icon = assetLoader.loadImage(
										`${ASSET_DATA_PATH}/img/${itemData.details.icon}.webp`
									)}
									<img
										src={icon || staticIcons.unknownIcon}
										alt={itemData.info.localized_name}
										class="h-5 w-5"
									/>
									<span>{itemData.info.localized_name}</span>
								{:else}
									<img src={staticIcons.unknownIcon} alt={material.id} class="h-5 w-5" />
									<span>{material.id}</span>
								{/if}
								<span class="ml-auto">{material.count}</span>
							</div>
						{/each}
						<div class="border-surface-600 flex items-center space-x-2 border-t pt-2 text-sm">
							<img src={staticIcons.workSpeedIcon} alt="Workload" class="h-5 w-5" />
							<span>Workload</span>
							<span class="ml-auto">{research.details.work_amount}</span>
						</div>
					</div>
				{/if}
			{:else}
				<p class="text-surface-500 text-center">Select a research node to see details.</p>
			{/if}
		</Card>
	{/if}
</div>

<style>
	.research-tree-container::-webkit-scrollbar {
		width: 8px;
		height: 8px;
	}
	.research-tree-container::-webkit-scrollbar-track {
		background: var(--color-surface-800);
		border-radius: 4px;
	}
	.research-tree-container::-webkit-scrollbar-thumb {
		background: var(--color-surface-600);
		border-radius: 4px;
	}
	.research-tree-container::-webkit-scrollbar-thumb:hover {
		background: var(--color-surface-500);
	}
</style>
