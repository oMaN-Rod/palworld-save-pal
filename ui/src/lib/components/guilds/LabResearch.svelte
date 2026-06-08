<script lang="ts">
	import { labResearchData } from '$lib/data';
	import { send } from '$lib/utils/websocketUtils';
	import { getToastState } from '$states';
	import { type Guild, type TreeNode, EntryState, MessageType } from '$types';
	import { debounce, deepCopy } from '$utils';
	import { onMount } from 'svelte';
	import ResearchNode from './ResearchNode.svelte';
	import ResearchDetailPanel from './ResearchDetailPanel.svelte';
	import { buildTree, findNodeById, getRequiredWork } from './researchTreeBuilder';
	import * as m from '$i18n/messages';

	let {
		guild = $bindable(),
		selectedCategory = $bindable('Handiwork')
	}: {
		guild: Guild;
		selectedCategory: string;
	} = $props();

	const toast = getToastState();

	let selectedNode: TreeNode | null = $state(null);
	let researchTree: Record<string, TreeNode[]> = $state({});
	let categories: string[] = $state([]);
	let nodeElements: { [key: string]: HTMLElement } = $state({});
	let lineCoords: { [key: string]: { x1: number; y1: number; x2: number; y2: number } } = $state(
		{}
	);

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
				m.unlock_prerequisite_entity({
					entity:
						labResearchData.getByKey(node.research.details.require_research_id)?.localized_name ||
						'Unknown'
				}),
				m.locked(),
				'warning'
			);
			return;
		}

		const updatedResearchList = deepCopy(guild.lab_research_data || []);
		const prerequisitesToUnlock: string[] = [];

		const findAndUnlockPrerequisites = (researchId: string) => {
			const researchDef = labResearchData.getByKey(researchId);
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
					newTree[cat] = buildTree(labResearchData.research, cat, guild);
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
		<svg class="pointer-events-none absolute top-0 left-0 z-0 w-full overflow-visible">
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
				<span>{m.select_a_entity({ entity: m.category() })}</span>
			{/if}
		</div>
	</div>

	{#if selectedNode}
		<ResearchDetailPanel {selectedNode} />
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
