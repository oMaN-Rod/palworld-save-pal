<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { cn } from '$theme';
	import * as m from '$i18n/messages';
	import type { GamePlayerJson } from '$states/gameState.svelte';

	let {
		players = [],
		selectedUid = null,
		onSelect
	}: {
		players?: GamePlayerJson[];
		selectedUid?: string | null;
		onSelect: (player: GamePlayerJson) => void;
	} = $props();
</script>

<div class="flex flex-col gap-2">
	{#each players as player (player.uid)}
		<button
			type="button"
			class={cn(
				'bg-surface-800 hover:bg-surface-700 flex w-full items-center justify-between gap-3 rounded-sm p-3 text-left',
				selectedUid === player.uid ? 'ring-secondary-500 ring-2' : ''
			)}
			onclick={() => onSelect(player)}
		>
			<div class="min-w-0">
				<p class="truncate font-medium">{player.nickname ?? player.uid}</p>
				<p class="text-surface-400 truncate text-xs">
					{#if player.level != null}{m.level()}
						{player.level}{/if}
					{#if player.exp != null}
						· {m.live_exp_total({ exp: player.exp })}{/if}
				</p>
			</div>
			<Icon icon="tabler:chevron-right" size={16} class="text-surface-400 shrink-0" />
		</button>
	{/each}
</div>
