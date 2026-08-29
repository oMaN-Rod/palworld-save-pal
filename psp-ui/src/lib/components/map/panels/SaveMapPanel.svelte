<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { PlayerList } from '$components/player';
	import { Combobox } from '$components/ui';
	import { getAppState } from '$states';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import type { ValueChangeDetails } from '@zag-js/accordion';
	import { worldToMap } from '../geo/utils';
	import { mapImg } from '../style/styles';
	import type { Base, GuildSummary, Player } from '$types';
	import * as m from '$i18n/messages';

	let {
		hideUnlockedFastTravel,
		hideCollectedRelics,
		showPlayers,
		showBases,
		selectedPlayerUid,
		onToggleHideUnlocked,
		onToggleHideCollected,
		onPlayerLoaded,
		onPlayerFocus,
		onBaseFocus,
		onEditBase
	}: {
		hideUnlockedFastTravel: boolean;
		hideCollectedRelics: boolean;
		showPlayers: boolean;
		showBases: boolean;
		selectedPlayerUid: string;
		onToggleHideUnlocked: () => void;
		onToggleHideCollected: () => void;
		onPlayerLoaded: (player: Player) => void;
		onPlayerFocus: (player: Player) => void;
		onBaseFocus: (base: Base) => void;
		onEditBase: (base: Base) => void;
	} = $props();

	const appState = getAppState();
	let selectedGuildId = $state('');
	let section = $state(['players']);

	const players = $derived(Object.values(appState.players || {}));
	const loadedPlayerCount = $derived(players.length);
	const guilds = $derived(Object.values(appState.guilds || {}));

	const bases = $derived.by(() =>
		guilds.reduce(
			(acc, guild) => {
				if (guild.bases) {
					Object.values(guild.bases).forEach((base) => {
						acc[base.id] = base;
					});
				}
				return acc;
			},
			{} as Record<string, Base>
		)
	);
	const loadedBaseCount = $derived(Object.keys(bases).length);

	const guildSelectOptions = $derived.by(() =>
		Object.entries((appState.guildSummaries ?? {}) as Record<string, GuildSummary>).map(
			([id, summary]) => ({
				value: id,
				label: summary.loaded
					? `■ ${summary.name} (${summary.base_count} bases)`
					: `□ ${summary.name} (${summary.base_count} bases)`
			})
		)
	);

	function handleGuildSelect(guildId: string) {
		selectedGuildId = guildId;
		const guild = appState.guilds?.[guildId];
		if (guild) {
			const firstBase = guild.bases ? Object.values(guild.bases)[0] : null;
			if (firstBase?.location) onBaseFocus(firstBase);
		} else {
			appState.loadGuildLazy(guildId);
		}
	}
</script>

{#if appState.selectedPlayer}
	<div class="border-surface-700 grid grid-cols-2 gap-2 rounded-sm border p-2">
		<button
			class="flex items-center space-x-2 {hideUnlockedFastTravel ? '' : 'opacity-25'}"
			onclick={onToggleHideUnlocked}
		>
			<img src={mapImg.fastTravel} alt={m.fast_travel()} class="mr-1 h-5 w-5" />
			<span class="truncate text-xs">{m.hide_unlocked()}</span>
		</button>
		<button
			class="flex items-center space-x-2 {hideCollectedRelics ? '' : 'opacity-25'}"
			onclick={onToggleHideCollected}
		>
			<img src={mapImg.effigy} alt={m.relics()} class="mr-1 h-5 w-5" />
			<span class="truncate text-xs">{m.hide_collected()}</span>
		</button>
	</div>
{/if}

<div class="flex flex-col gap-2">
	<div class="flex items-center gap-2">
		<Icon icon="tabler:users" class="h-4 w-4" />
		<span class="text-sm font-medium">{m.load_player()}</span>
	</div>
	<PlayerList selected={selectedPlayerUid} onselect={onPlayerLoaded} redirect={false} />
</div>

<div class="flex flex-col gap-2">
	<div class="flex items-center gap-2">
		<Icon icon="tabler:building" class="h-4 w-4" />
		<span class="text-sm font-medium">{m.load_guild_bases()}</span>
	</div>
	{#if appState.loadingGuild}
		<div class="text-surface-400 my-2 flex items-center gap-2 px-3 py-2 text-sm">
			<svg class="h-4 w-4 animate-spin" viewBox="0 0 24 24">
				<circle
					class="opacity-25"
					cx="12"
					cy="12"
					r="10"
					stroke="currentColor"
					stroke-width="4"
					fill="none"
				></circle>
				<path
					class="opacity-75"
					fill="currentColor"
					d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
				></path>
			</svg>
			{m.loading_entity({ entity: m.guild({ count: 1 }) })}
		</div>
	{:else}
		<Combobox
			value={selectedGuildId}
			options={guildSelectOptions}
			placeholder={m.select_entity({ entity: m.guild({ count: 1 }) })}
			onChange={(value) => handleGuildSelect(value as string)}
			selectClass="w-full"
		/>
	{/if}
	<p class="text-surface-500 text-xs">{m.select_guild_to_load_bases()}</p>
</div>

<Accordion
	value={section}
	onValueChange={(e: ValueChangeDetails) => (section = e.value)}
	collapsible
>
	{#if showPlayers}
		<Accordion.Item value="players" controlHover="hover:bg-secondary-500/25">
			{#snippet control()}
				<h2 class="text-lg font-bold">
					{m.loaded_entity({ entity: m.player({ count: 2 }) })}
				</h2>
			{/snippet}
			{#snippet panel()}
				{#if loadedPlayerCount > 0}
					<div class="max-h-64 space-y-2 overflow-y-auto">
						{#each players as player}
							{#if player.location}
								{@const mapCoords = worldToMap(player.location.x, player.location.y)}
								<button
									class="bg-surface-800 hover:bg-secondary-500/25 w-full rounded-sm p-2 text-start"
									onclick={() => onPlayerFocus(player)}
								>
									<div class="truncate font-bold">{player.nickname}</div>
									<div class="text-xs">
										{m.level()}: {player.level} | {m.hp()}: {player.hp}
									</div>
									<div class="text-surface-400 text-xs">
										{m.location()}: {Math.round(mapCoords.x)}, {Math.round(mapCoords.y)}
									</div>
									<div class="text-surface-400 text-xs">
										{m.last_online()}: {new Date(player.last_online_time).toLocaleString()}
									</div>
								</button>
							{/if}
						{/each}
					</div>
				{:else}
					<p class="text-surface-500 text-sm">
						{m.no_players_loaded()}
					</p>
				{/if}
			{/snippet}
		</Accordion.Item>
	{/if}
	{#if showBases}
		<Accordion.Item value="bases" controlHover="hover:bg-secondary-500/25">
			{#snippet control()}
				<h2 class="text-lg font-bold">
					{m.loaded_entity({ entity: m.base({ count: 2 }) })}
				</h2>
			{/snippet}
			{#snippet panel()}
				{#if loadedBaseCount > 0}
					<div class="max-h-64 space-y-2 overflow-y-auto">
						{#each Object.values(bases) as base}
							{#if base.location}
								<button
									class="bg-surface-800 hover:bg-secondary-500/25 mb-2 w-full rounded-sm p-2 text-start"
									onclick={() => onBaseFocus(base)}
									oncontextmenu={(e) => {
										e.preventDefault();
										onEditBase(base);
									}}
								>
									<div class="truncate font-bold">{base.name}</div>
									<div class="text-surface-400 text-xs">
										{m.id()}: {base.id}
									</div>
									<div class="text-surface-400 text-xs">
										{m.location()}: {worldToMap(base.location.x, base.location.y).x}, {worldToMap(
											base.location.x,
											base.location.y
										).y}
									</div>
								</button>
							{/if}
						{/each}
					</div>
				{:else}
					<p class="text-surface-500 text-sm">
						{m.no_bases_loaded()}
					</p>
				{/if}
			{/snippet}
		</Accordion.Item>
	{/if}
</Accordion>
