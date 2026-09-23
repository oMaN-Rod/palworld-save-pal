<script lang="ts">
	import { List } from '$components/ui';
	import { buildingsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { staticIcons } from '$types/icons';
	import type { ItemContainer, ItemContainerSlot } from '$types';
	import * as m from '$i18n/messages';

	import GuildContainerGrid from './GuildContainerGrid.svelte';

	interface Props {
		containers: ItemContainer[];
		selected: (ItemContainer & { slots: ItemContainerSlot[] }) | undefined;
		onSelect: (container: ItemContainer) => void;
		onUpdate: () => void;
		onCopyPaste: (event: MouseEvent, slot: ItemContainerSlot) => void;
	}

	let { containers, selected, onSelect, onUpdate, onCopyPaste }: Props = $props();
</script>

{#if containers.length > 0}
	<div id="guild-storage-content" class="flex flex-col gap-4 md:flex-row md:gap-0 md:space-x-4">
		<div id="guild-storage-containers" class="w-full shrink-0 md:w-1/4">
			<List
				items={containers}
				baseClass="w-full"
				listClass="max-h-64 md:h-[calc(100vh-var(--titlebar-h)-175px)] md:max-h-none"
				canSelect={false}
				idKey="id"
				onselect={(container) => onSelect(container)}
				multiple={false}
			>
				{#snippet listItem(item)}
					{@const building = buildingsData.getByKey(item.key)}
					{#if building}
						{@const buildingIcon = assetLoader.loadImage(
							`${ASSET_DATA_PATH}/img/${building.icon}.webp`
						)}
						<div class="grid min-w-0 grid-cols-[auto_1fr] gap-2">
							<img
								src={buildingIcon || staticIcons.unknownIcon}
								alt={building.localized_name}
								class="h-8 w-8 shrink-0"
							/>
							<span class="truncate">{building.localized_name}</span>
						</div>
					{:else}
						<div class="grid min-w-0 grid-cols-[auto_1fr] gap-2">
							<img src={staticIcons.unknownIcon} alt={item.key} class="h-8 w-8 shrink-0" />
							<span class="truncate">{item.key}</span>
						</div>
					{/if}
				{/snippet}
				{#snippet listItemPopup(item)}
					{@const building = buildingsData.getByKey(item.key)}
					{#if building}
						<div class="flex flex-col">
							<h4 class="h4">{building.localized_name}</h4>
							<div class="grid w-full grid-cols-2 gap-2">
								<span class="font-bold">{m.available_slots()}</span>
								<span>{item.slot_num}</span>
							</div>
							<div class="grid w-full grid-cols-2 gap-2">
								<span class="font-bold">{m.used_slots()}</span>
								<span>
									{item?.slots?.filter((slot) => slot.static_id !== 'None').length}
								</span>
							</div>
						</div>
					{:else}
						{item.key}
					{/if}
				{/snippet}
			</List>
		</div>
		<div
			class="min-w-0 flex-1 overflow-y-auto max-md:max-h-[60vh] md:max-h-[calc(100vh-var(--titlebar-h)-450px)] 2xl:max-h-[calc(100vh-var(--titlebar-h)-200px)]"
		>
			{#if selected}
				<GuildContainerGrid container={selected} {onUpdate} {onCopyPaste} />
			{:else}
				<div class="flex w-full items-center justify-center">
					<h2 class="h2">{m.select_entity({ entity: m.storage_container() })}</h2>
				</div>
			{/if}
		</div>
	</div>
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">{m.no_storage_containers()}</h2>
	</div>
{/if}
