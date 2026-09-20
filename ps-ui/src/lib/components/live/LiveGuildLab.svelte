<script lang="ts">
	import { labResearchData } from '$lib/data/labResearch.svelte';
	import { LabResearchControls, ResearchDetailPanel, ResearchNode } from '$components/guilds';
	import { buildTree } from '$components/guilds/researchTreeBuilder';
	import { countResearched, toResearchGuildShape } from './liveGuild.utils';
	import type { GameGuildLabJson } from '$states/gameState.svelte';
	import type { Guild, TreeNode } from '$types';
	import * as m from '$i18n/messages';

	let {
		lab = null,
		setResearchReason
	}: {
		lab?: GameGuildLabJson | null;
		setResearchReason?: string;
	} = $props();

	let selectedCategory = $state('Handcraft');
	let selectedNode = $state<TreeNode | null>(null);
	let nodeElements = $state<{ [key: string]: HTMLElement }>({});

	const shape = $derived(toResearchGuildShape(lab) as unknown as Guild);
	const counts = $derived(countResearched(lab, labResearchData.research));
	const roots = $derived(
		Object.keys(labResearchData.research).length > 0
			? buildTree(labResearchData.research, selectedCategory, shape)
			: []
	);
</script>

<div class="flex flex-col gap-2">
	<p class="text-surface-400 text-xs">
		{m.live_guild_research_progress({ done: counts.done, total: counts.total })}
	</p>

	<div class="grid gap-3 @lg/guild:grid-cols-[20rem_minmax(0,1fr)_20rem]">
		<LabResearchControls bind:selectedCategory guild={shape} />

		<div id="live-guild-lab" class="flex min-w-max flex-col items-center gap-6 overflow-x-auto p-2">
			{#each roots as rootNode (rootNode.id)}
				<ResearchNode
					node={rootNode}
					{selectedNode}
					selectNode={(node: TreeNode) => (selectedNode = node)}
					showProgress
					bind:nodeElements
				/>
			{/each}
		</div>

		{#if selectedNode}
			<div class="flex-col gap-2 hidden @lg/guild:flex">
				<ResearchDetailPanel {selectedNode} />
				<p class="text-surface-500 text-xs">
					{setResearchReason ?? m.live_guild_research_read_only()}
				</p>
			</div>
		{/if}
	</div>
</div>
