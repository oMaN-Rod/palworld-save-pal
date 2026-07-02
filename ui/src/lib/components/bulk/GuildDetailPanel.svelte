<script lang="ts">
	import { List, Loading } from '$components/ui';
	import { getAppState } from '$states';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { X } from 'lucide-svelte';
	import { Pencil, Users } from '@lucide/svelte';
	import Input from '$components/ui/input/Input.svelte';

	let { expanded = false, onclose }: { expanded?: boolean; onclose?: () => void } = $props();

	const appState = getAppState();
	const guild = $derived(appState.bulkDetailGuild);
	const guildPlayers = $derived(guild ? Object.entries(appState.playerSummaries).filter(([uid]) => guild.players?.includes(uid)) : undefined);

	let query = $state('');
	const filteredPlayers = $derived.by(() => {
		if (!guildPlayers) return [];
		return guildPlayers.filter(([uid, player]) => {
			const search = query.toLowerCase();
			return (
				player.nickname?.toLowerCase().includes(search) ||
				uid.toLowerCase().includes(search)
			);
		});
	});

	function memberName(uid: string): string {
		return appState.playerSummaries[uid]?.nickname ?? uid;
	}

	function editPlayer(uid: string) {
		appState.selectPlayerLazy(uid); // navigates to /edit/player
	}
</script>

<div
	class="bg-surface-800/80 text-on-surface h-[calc(100vh-84px)] shrink-0 overflow-hidden shadow-lg backdrop-blur-md transition-all duration-300 ease-in-out"
	style:width={expanded ? '420px' : '0px'}
>
	<div class="flex h-full w-105 flex-col overflow-y-auto p-4">
		<div class="mb-3 flex items-center justify-between">
			<span class="font-semibold">{c.guild}</span>
			<button
				class="hover:text-primary-500 rounded p-1"
				onclick={() => onclose?.()}
				aria-label={m.close_drawer()}
			>
				<X class="h-4 w-4" />
			</button>
		</div>
		{#if appState.loadingGuild}
			<div class="flex flex-1 items-center justify-center">
				<Loading
					label={m.loading_entity({ entity: c.guild })}
					loadingComplete={!appState.loadingGuild}
					icon={Users}
				/>
			</div>
		{:else if guild}
			<div class="flex flex-col gap-3">
				<h3 class="h4">{guild.name}</h3>
				<dl class="grid grid-cols-2 gap-1 text-sm">
					<dt>{m.level()}</dt>
					<dd>{guild.base_camp_level ?? '—'}</dd>
					<dt>{c.players}</dt>
					<dd>{guild.players?.length ?? 0}</dd>
					<dt>{c.bases}</dt>
					<dd>{guild.bases ? Object.keys(guild.bases).length : 0}</dd>
				</dl>
				<div class="flex flex-col gap-1">
					<h4 class="text-sm font-semibold">{c.players}</h4>
					<List items={filteredPlayers} canSelect={false} class="flex flex-col gap-1" headerClass="flex p-0">
						{#snippet listHeader()}
							{#if filteredPlayers.length > 5}
								<Input bind:value={query} inputClass="my-0" placeholder={m.search_entity({ entity: c.players })} />
							{:else}
								<div></div>
							{/if}
						{/snippet}
						{#snippet listItem([playerUid, player])}
							<div class="flex gap-2">
								<span class="font-bold">Lvl {player?.level ?? '—'}</span>
								<span class="truncate">{player?.nickname ?? playerUid}</span>
							</div>
							
						{/snippet}
						{#snippet listItemActions([playerUid, player])}
							<button
								class="ml-2 text-left text-sm hover:underline"
								onclick={() => editPlayer(playerUid)}
							>
								<Pencil class="h-4 w-4" />
							</button>
						{/snippet}
					</List>
				</div>
			</div>
		{:else}
			<div class="flex flex-1 items-center justify-center">
				<p class="text-surface-400 text-sm">
					{m.failed_load_entity({ entity: c.guild })}
				</p>
			</div>
		{/if}
	</div>
</div>
