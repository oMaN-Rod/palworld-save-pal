<script lang="ts">
	import { Tabs } from '@skeletonlabs/skeleton-svelte';
	import type { ValueChangeDetails } from '@zag-js/tabs';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { getAppState } from '$states';
	import PlayersTable from '$lib/components/bulk/PlayersTable.svelte';
	import GuildsTable from '$lib/components/bulk/GuildsTable.svelte';
	import PalsTable from '$lib/components/bulk/PalsTable.svelte';

	let selectedTab = $state('players');
	let playerSelection = $state(new Set<string>());
	let guildSelection = $state(new Set<string>());
	let palSelection = $state(new Set<string>());

	const appState = getAppState();
	const playerCount = $derived(appState.playerSummariesArray.length);
	const guildCount = $derived(appState.guildSummariesArray.length);
	const baseCount = $derived(
		appState.guildSummariesArray.reduce((total, guild) => total + guild.base_count, 0)
	);
	const palCount = $derived(
		appState.playerSummariesArray.reduce((total, player) => total + player.pal_count, 0)
	);
</script>

<div class="flex h-full flex-col gap-4 p-4">
	<header class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 px-1">
		<h1 class="heading-gradient text-xl font-extrabold tracking-tight sm:text-2xl">
			{m.entity_registry()}
		</h1>
		<p class="text-surface-400 text-xs">
			{playerCount}
			{c.players} · {guildCount}
			{c.guilds} · {palCount}
			{c.pals} · {baseCount}
			{c.bases}
		</p>
	</header>
	<Tabs value={selectedTab} onValueChange={(e: ValueChangeDetails) => (selectedTab = e.value)}>
		{#snippet list()}
			<Tabs.Control value="players">{c.players}</Tabs.Control>
			<Tabs.Control value="pals">{c.pals}</Tabs.Control>
			<Tabs.Control value="guilds">{c.guilds}</Tabs.Control>
		{/snippet}
		{#snippet content()}
			<Tabs.Panel value="players">
				<PlayersTable bind:selected={playerSelection} />
			</Tabs.Panel>
			<Tabs.Panel value="pals">
				<PalsTable bind:selected={palSelection} />
			</Tabs.Panel>
			<Tabs.Panel value="guilds">
				<GuildsTable bind:selected={guildSelection} />
			</Tabs.Panel>
		{/snippet}
	</Tabs>
</div>
