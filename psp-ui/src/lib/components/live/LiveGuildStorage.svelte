<script lang="ts">
	import { Spinner } from '$components/ui';
	import { ItemBadge } from '$components/shared';
	import { containerSlots } from './liveView.utils';
	import LivePalboxPager from './LivePalboxPager.svelte';
	import { buildingsData } from '$lib/data';
	import { assetLoader } from '$utils';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { staticIcons } from '$types/icons';
	import { cn } from '$theme';
	import type {
		GameGuildBaseJson,
		GameInventoryContainerJson,
		GameRefusalJson
	} from '$states/gameState.svelte';
	import type { ItemContainerSlot } from '$types';
	import * as m from '$i18n/messages';

	let {
		containers = [],
		error = null,
		loading = false,
		bases = [],
		basePage = 0,
		onBasePageChange,
		onSetSlot,
		editDisabledReason
	}: {
		containers?: GameInventoryContainerJson[];
		error?: GameRefusalJson | null;
		loading?: boolean;
		bases?: GameGuildBaseJson[];
		basePage?: number;
		onBasePageChange?: (page: number) => void;
		onSetSlot?: (
			containerId: string,
			slotIndex: number,
			staticItemId: string | null,
			count: number
		) => void;
		editDisabledReason?: string;
	} = $props();

	const ignoreKeys = ['None', 'Empty', 'TreasureBox', 'PalEgg', 'CommonDropItem'];

	function buildingFor(container: GameInventoryContainerJson) {
		return container.buildObjectId ? buildingsData.getByKey(container.buildObjectId) : undefined;
	}

	function buildingName(container: GameInventoryContainerJson): string {
		return buildingFor(container)?.localized_name ?? container.buildObjectId ?? m.storage();
	}

	const currentBaseId = $derived(bases[basePage]?.id ?? null);

	const filtered = $derived(
		containers
			.filter((container) => container.baseId === currentBaseId)
			.filter((container) => container.slotNum !== 0)
			.filter((container) => !ignoreKeys.some((key) => (container.buildObjectId ?? '').includes(key)))
			.sort((a, b) => buildingName(a).localeCompare(buildingName(b)))
	);

	let selectedId = $state<string | null>(null);

	const selected = $derived(
		filtered.find((entry) => entry.containerId === selectedId) ?? filtered[0] ?? null
	);
	const slots = $derived(selected ? containerSlots(selected) : []);

	function usedCount(container: GameInventoryContainerJson): number {
		return container.slots.filter((slot) => slot.staticItemId !== 'None').length;
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
{:else}
	{#if bases.length > 1}
		<LivePalboxPager
			id="live-storage-base-pager"
			page={basePage}
			pageCount={bases.length}
			labelFor={(index) => bases[index]?.name ?? String(index + 1)}
			onPageChange={(page) => onBasePageChange?.(page)}
		/>
	{/if}
	{#if loading && filtered.length === 0}
		<div class="flex items-center gap-3 py-2">
			<Spinner size="size-8" />
			<span class="text-surface-300 text-sm">{m.loading()}</span>
		</div>
	{:else if filtered.length === 0}
		<p class="text-surface-400 text-sm">{m.no_entity_yet({ entity: m.storage() })}</p>
	{:else}
		<div id="live-guild-containers" class="grid gap-3 @lg/guild:grid-cols-[13rem_minmax(0,1fr)]">
			<ul class="divide-surface-800 border-surface-800 max-h-180 divide-y overflow-y-auto rounded-sm border">
				{#each filtered as container (container.containerId)}
					{@const building = buildingFor(container)}
					{@const icon = building
						? assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${building.icon}.webp`)
						: staticIcons.unknownIcon}
					<li>
						<button
							type="button"
							class={cn(
								'hover:bg-surface-800 flex w-full items-center gap-2 truncate px-2 py-1.5 text-left text-xs',
								selected?.containerId === container.containerId && 'bg-primary-900/40'
							)}
							onclick={() => (selectedId = container.containerId)}
						>
							<img src={icon || staticIcons.unknownIcon} alt="" class="h-8 w-8 shrink-0" />
							<span class="truncate">{buildingName(container)}</span>
							<span class="text-surface-400 shrink-0">
								({usedCount(container)}/{container.slotNum ?? 0})
							</span>
						</button>
					</li>
				{/each}
			</ul>

			<div data-testid="live-guild-container-slots" class="grid grid-cols-3 gap-2 sm:grid-cols-4 md:grid-cols-6">
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
{/if}
