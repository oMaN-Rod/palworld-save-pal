<script lang="ts">
	import SectionTabs, { panelId, tabId } from '$components/ui/tabs/SectionTabs.svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { getAppState } from '$states';
	import PlayersTable from '$lib/components/bulk/PlayersTable.svelte';
	import GuildsTable from '$lib/components/bulk/GuildsTable.svelte';
	import PalsTable from '$lib/components/bulk/PalsTable.svelte';
	import Icon from '$components/ui/icons/Icon.svelte';

	const SECTION_ID_PREFIX = 'registry';
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

	const tabs = $derived([
		{ id: 'players', label: c.players },
		{ id: 'pals', label: c.pals },
		{ id: 'guilds', label: c.guilds }
	]);
</script>

{#if appState.saveFile}
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
		<SectionTabs
			{tabs}
			bind:active={selectedTab}
			label={m.entity_registry()}
			idPrefix={SECTION_ID_PREFIX}
		/>

		<!-- One panel, not one per tab: each mounted table paginates the whole save. -->
		<div
			id={panelId(SECTION_ID_PREFIX, selectedTab)}
			role="tabpanel"
			aria-labelledby={tabId(SECTION_ID_PREFIX, selectedTab)}
			tabindex="0"
			class="min-h-0 flex-1"
		>
			{#if selectedTab === 'players'}
				<PlayersTable bind:selected={playerSelection} />
			{:else if selectedTab === 'pals'}
				<PalsTable bind:selected={palSelection} />
			{:else}
				<GuildsTable bind:selected={guildSelection} />
			{/if}
		</div>
	</div>
{:else}
	<div class="flex h-full w-full items-center justify-center">
		<h2 class="h2 flex items-center gap-2">
			<Icon icon="tabler:alert-circle" class="text-secondary-400 h-6 w-6" />
			{m.no_save_loaded()}
		</h2>
	</div>
{/if}
