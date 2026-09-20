<script lang="ts">
	import { Spinner } from '$components/ui';
	import { ItemBadge } from '$components/shared';
	import { containerSlots } from './liveView.utils';
	import { buildingsData } from '$lib/data/buildings.svelte';
	import { assetLoader } from '$utils';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { staticIcons } from '$types/icons';
	import { cn } from '$theme';
	import type { GameInventoryContainerJson, GameRefusalJson } from '$states/gameState.svelte';
	import type { ItemContainerSlot } from '$types';
	import * as m from '$i18n/messages';

	let {
		containers = [],
		error = null,
		loading = false,
		onSetSlot,
		editDisabledReason
	}: {
		containers?: GameInventoryContainerJson[];
		error?: GameRefusalJson | null;
		loading?: boolean;
		onSetSlot?: (
			containerId: string,
			slotIndex: number,
			staticItemId: string | null,
			count: number
		) => void;
		editDisabledReason?: string;
	} = $props();

	let selectedId = $state<string | null>(null);

	const selected = $derived(
		containers.find((entry) => entry.containerId === selectedId) ?? containers[0] ?? null
	);
	const slots = $derived(selected ? containerSlots(selected) : []);

	const building = $derived(buildingsData.getByKey('GuildChest'));
	const icon = $derived(
		building ? assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${building.icon}.webp`) : staticIcons.unknownIcon
	);

	function label(container: GameInventoryContainerJson): string {
		const used = container.slots.filter((slot) => slot.staticItemId !== 'None').length;
		const name = building?.localized_name ?? m.live_guild_view_chest();
		return `${name} (${used}/${container.slotNum ?? 0})`;
	}

	const editable = $derived(Boolean(onSetSlot) && !editDisabledReason);
	const canEdit = $derived(editable && !!selected?.containerId);

	function commit(slot: ItemContainerSlot): void {
		const containerId = selected?.containerId;
		if (!containerId || !onSetSlot) return;
		const staticItemId = slot.static_id === 'None' ? null : slot.static_id;
		onSetSlot(containerId, slot.slot_index, staticItemId, slot.count ?? 0);
	}
</script>

{#if error}
	<p class="text-error-400 text-sm">{error.error}</p>
{:else if loading && !selected}
	<div class="flex items-center gap-3 py-2">
		<Spinner size="size-8" />
		<span class="text-surface-300 text-sm">{m.loading()}</span>
	</div>
{:else if !selected}
	<p class="text-surface-400 text-sm">{m.live_guild_no_chest()}</p>
{:else}
	<div id="live-guild-chests" class="grid gap-3 @lg/guild:grid-cols-[13rem_minmax(0,1fr)]">
		<ul class="divide-surface-800 border-surface-800 max-h-80 divide-y overflow-y-auto rounded-sm border">
			{#each containers as container (container.containerId)}
				<li>
					<button
						type="button"
						class={cn(
							'hover:bg-surface-800 flex w-full items-center gap-2 truncate px-2 py-1.5 text-left text-xs',
							selected?.containerId === container.containerId && 'bg-primary-900/40'
						)}
						onclick={() => (selectedId = container.containerId)}
					>
						<img src={icon} alt="" class="h-6 w-6 shrink-0" />
						<span class="truncate">{label(container)}</span>
					</button>
				</li>
			{/each}
		</ul>

		<div data-testid="live-guild-chest-slots" class="grid grid-cols-3 gap-2 sm:grid-cols-4 md:grid-cols-6">
			{#each slots as slot (slot.slot_index)}
				<ItemBadge
					{slot}
					itemGroup="Common"
					disabled={!canEdit}
					onUpdate={(updated) => commit(updated)}
				/>
			{/each}
		</div>
	</div>
{/if}
