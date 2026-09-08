<script lang="ts">
	import { Input, Popover } from '$components/ui';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import LivePlayerList from './LivePlayerList.svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import type { GamePlayerJson } from '$states/gameState.svelte';

	let {
		players = [],
		selected,
		onSelect
	}: {
		players?: GamePlayerJson[];
		selected: GamePlayerJson;
		onSelect: (player: GamePlayerJson) => void;
	} = $props();

	let query = $state('');

	const matches = $derived.by(() => {
		const needle = query.trim().toLowerCase();
		if (!needle) return players;
		return players.filter((player) =>
			(player.nickname ?? player.uid).toLowerCase().includes(needle)
		);
	});
</script>

<Popover position="bottom-start" popoverClass="w-72 max-h-96 overflow-y-auto">
	<button
		type="button"
		id="live-player-switcher"
		aria-label={m.live_switch_player()}
		class="bg-surface-800 hover:bg-surface-700 flex items-center gap-2 rounded-sm py-1 pr-2 pl-3 min-w-48"
	>
		<span class="min-w-0 text-left grow">
			<span class="block truncate text-sm font-semibold">
				{selected.nickname ?? selected.uid}
			</span>
			{#if selected.level != null}
				<span class="text-surface-400 block text-xs">{m.level()} {selected.level}</span>
			{/if}
		</span>
		<Icon icon="tabler:chevron-down" size={16} class="text-surface-400 shrink-0" />
	</button>

	{#snippet content({ close }: { close: () => void })}
		<div class="flex flex-col gap-2">
			<Input
				type="search"
				placeholder={m.search()}
				value={query}
				oninput={(event: Event) => (query = (event.currentTarget as HTMLInputElement).value)}
			/>
			{#if matches.length === 0}
				<p class="text-surface-400 px-1 text-sm">
					{m.no_entity_matching({ entity: c.players, query })}
				</p>
			{:else}
				<LivePlayerList
					players={matches}
					selectedUid={selected.uid}
					onSelect={(player) => {
						query = '';
						close();
						onSelect(player);
					}}
				/>
			{/if}
		</div>
	{/snippet}
</Popover>
