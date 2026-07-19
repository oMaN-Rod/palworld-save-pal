<script lang="ts">
	import { TextInputModal } from '$components/modals';
	import { Button, List, TooltipButton } from '$components/ui';
	import { itemsData, presetsData } from '$lib/data';
	import type { ItemContainer, ItemContainerSlot, PresetProfile } from '$lib/types';
	import { getModalState, sortPresets } from '$states';
	import { EntryState } from '$types';
	import { deepCopy } from '$utils';
	import {
		Edit,
		PaintBucket,
		Play,
		Plus,
		Trash,
		X,
		Hash,
		ChevronsLeftRight,
		PackagePlus
	} from 'lucide-svelte';
	import { ItemSelectModal, NumberInputModal } from '$components/modals';
	import * as m from '$i18n/messages';
	import { c } from '$utils/commonTranslations';

	let { container, onUpdate }:{
		container: ItemContainer & { slots: ItemContainerSlot[] };
		onUpdate: () => void;
	} = $props();

	const modal = getModalState();

	type ExtendedPresetProfile = PresetProfile & { id: string };

	let selectedPreset: ExtendedPresetProfile = $state({ id: '', name: '', type: 'storage' });
	let selectedPresets: ExtendedPresetProfile[] = $state([]);
	let selectAll: boolean = $state(false);

	let filteredPresets: ExtendedPresetProfile[] = $derived.by(() => {
		return sortPresets(
			Object.entries(presetsData.presetProfiles)
				.filter(
					([_, preset]) =>
						preset.type === 'storage' &&
						preset.storage_container &&
						(preset.storage_container.slots?.length ?? 0) <= container.slots.length
				)
				.map(([id, preset]) => ({ ...preset, id })),
			'storage'
		);
	});

	async function handleApplyPreset() {
		if (!selectedPresets.length || !container) return;

		// Merge the items from every selected preset into one flat list.
		const allPresetSlots: ItemContainerSlot[] = [];
		for (const preset of selectedPresets) {
			if (preset.storage_container) {
				for (const ps of preset.storage_container.slots) {
					if (ps.static_id !== 'None') {
						allPresetSlots.push(ps as ItemContainerSlot);
					}
				}
			}
		}

		// Overwrite mode: First empty the container, then fill in
		// the preset items in sequence.
		const updatedSlots = container.slots.map((slot: ItemContainerSlot, idx: number) => {
			if (idx < allPresetSlots.length) {
				const presetSlot = allPresetSlots[idx];
				let dynamic_item = undefined;
				if (presetSlot.dynamic_item) {
					dynamic_item = deepCopy(presetSlot.dynamic_item);
					dynamic_item.local_id = '00000000-0000-0000-0000-000000000000';
				}
				return {
					...slot,
					static_id: presetSlot.static_id,
					count: presetSlot.count,
					dynamic_item
				};
			}
			return { ...slot, static_id: 'None', count: 0, dynamic_item: undefined };
		});

		container.slots = updatedSlots;
		container.state = EntryState.MODIFIED;
		onUpdate();
		selectedPresets = [];
	}

	async function handleAppendPreset() {
		if (!selectedPresets.length || !container) return;

		// Merge the items from every selected preset into one flat list.
		const allPresetSlots: ItemContainerSlot[] = [];
		for (const preset of selectedPresets) {
			if (preset.storage_container) {
				for (const ps of preset.storage_container.slots) {
					if (ps.static_id !== 'None') {
						allPresetSlots.push(ps as ItemContainerSlot);
					}
				}
			}
		}

		// Collect the empty slots (static_id === 'None') in the container.
		const emptySlots = container.slots.filter((s: ItemContainerSlot) => s.static_id === 'None');

		// Fill the empty slots in order; discard anything beyond capacity.
		let emptyIdx = 0;
		for (const presetSlot of allPresetSlots) {
			if (emptyIdx >= emptySlots.length) break;

			const targetSlot = emptySlots[emptyIdx];
			targetSlot.static_id = presetSlot.static_id;
			targetSlot.count = presetSlot.count;
			if (presetSlot.dynamic_item) {
				targetSlot.dynamic_item = deepCopy(presetSlot.dynamic_item);
				targetSlot.dynamic_item.local_id = '00000000-0000-0000-0000-000000000000';
			} else {
				targetSlot.dynamic_item = undefined;
			}
			emptyIdx++;
		}

		container.state = EntryState.MODIFIED;
		onUpdate();
		selectedPresets = [];
	}

	function processSlots(slots: ItemContainerSlot[]) {
		const newSlots = deepCopy(slots);
		return newSlots
			.map((slot) => {
				if (slot.dynamic_item) {
					slot.dynamic_item.local_id = '00000000-0000-0000-0000-000000000000';
					return { ...slot };
				}
				return slot;
			})
			.filter((slot) => slot.static_id !== 'None');
	}

	async function handleAddPreset() {
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: m.add_preset_entity({ entity: m.storage() }),
			value: ''
		});
		if (!result) return;

		const newPreset = {
			name: result,
			type: 'storage',
			storage_container: {
				key: container.key,
				slots: processSlots(container.slots)
			}
		} as PresetProfile;

		await presetsData.addPresetProfile(newPreset);
		selectedPresets = [];
		selectAll = false;
	}

	async function handleDeletePresets() {
		if (selectedPresets.length === 0) return;
		const result = await modal.showConfirmModal({
			title: m.delete_entity({ entity: c.preset }),
			message: m.delete_count_entities_confirm({
				count: selectedPresets.length,
				entity: m.preset({ count: selectedPresets.length })
			})
		});
		if (!result) return;
		const presetIds = selectedPresets.map((preset) => preset.id);
		await presetsData.removePresetProfiles(presetIds);
		selectedPresets = [];
		selectAll = false;
	}

	async function handleEditPresetName(preset: ExtendedPresetProfile) {
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: m.edit_entity({ entity: c.preset }),
			value: preset.name
		});
		if (!result) return;
		await presetsData.changePresetName(preset.id, result);
	}

	async function handleFillContainer() {
		// @ts-ignore
		const result = await modal.showModal<[string, number]>(ItemSelectModal, {
			group: 'Common',
			itemId: '',
			title: m.search_entity({ entity: c.item })
		});
		if (!result) return;
		let [static_id, count] = result;
		const itemData = itemsData.getByKey(static_id);
		if (!itemData) return;
		count = count > itemData.details.max_stack_count ? itemData.details.max_stack_count : count;

		container.slots.forEach((slot: ItemContainerSlot) => {
			slot.static_id = static_id;
			slot.count = count;
			if (itemData.details.dynamic) {
				// @ts-ignore
				slot.dynamic_item = {
					local_id: '00000000-0000-0000-0000-000000000000',
					durability: itemData.details.dynamic.durability || 0,
					remaining_bullets: itemData.details.dynamic.magazine_size || 0,
					type: itemData.details.dynamic.type
				};
			} else {
				slot.dynamic_item = undefined;
			}
		});
		container.state = EntryState.MODIFIED;
	}

	async function handleSetContainerCount() {
		// @ts-ignore
		const result = await modal.showModal<number>(NumberInputModal, {
			title: m.enter_item_count(),
			value: '',
			min: 0,
			max: 9999
		});
		if (!result) return;

		container.slots.forEach((slot: ItemContainerSlot) => {
			if (slot.static_id === 'None') return;
			slot.count = result;
		});
		container.state = EntryState.MODIFIED;
	}

	function handleClearContainer() {
		container.slots.forEach((slot: ItemContainerSlot) => {
			slot.dynamic_item = undefined;
			slot.static_id = 'None';
			slot.count = 0;
			// @ts-ignore
			slot.local_id = '00000000-0000-0000-0000-000000000000';
		});
		container.state = EntryState.MODIFIED;
	}
</script>

<div class="flex w-full max-w-96 min-w-0 flex-col space-y-2">
	<div class="btn-group bg-surface-900 items-center rounded-sm p-1">
		<TooltipButton
			onclick={handleAddPreset}
			popupLabel={m.create_preset_from_current({ entity: c.container })}
		>
			<Plus />
		</TooltipButton>
		<TooltipButton
			onclick={handleFillContainer}
			popupLabel={m.fill_current_entity({ entity: c.container })}
		>
			<PaintBucket />
		</TooltipButton>
		<TooltipButton
			onclick={handleSetContainerCount}
			popupLabel={m.set_entity_item_count({ entity: c.container })}
		>
			<Hash />
		</TooltipButton>
		<TooltipButton
			onclick={handleClearContainer}
			popupLabel={m.clear_entity({ entity: c.container })}
		>
			<ChevronsLeftRight />
		</TooltipButton>
		{#if selectedPresets.length >= 1}
			<TooltipButton
				onclick={handleApplyPreset}
				popupLabel={m.apply_selected_entity({ entity: c.preset })}
			>
				<Play />
			</TooltipButton>
			<TooltipButton onclick={handleAppendPreset} popupLabel={m.append_to_empty_slots()}>
				<PackagePlus />
			</TooltipButton>
			<TooltipButton
				onclick={handleDeletePresets}
				popupLabel={m.delete_selected_entity({
					entity: m.preset({ count: selectedPresets.length })
				})}
			>
				<Trash />
			</TooltipButton>
			<TooltipButton onclick={() => (selectedPresets = [])} popupLabel={m.clear_selected()}>
				<X />
			</TooltipButton>
		{/if}
	</div>
	<List
		baseClass="bg-surface-800"
		items={filteredPresets}
		bind:selectedItems={selectedPresets}
		bind:selectedItem={selectedPreset}
		bind:selectAll
		onlyHighlightChecked
	>
		{#snippet listHeader()}
			<div class="flex justify-start">
				<span class="font-bold">{c.preset}</span>
			</div>
		{/snippet}
		{#snippet listItem(preset)}
			<span class="grow">{preset.name}</span>
		{/snippet}
		{#snippet listItemActions(preset)}
			<Button variant="ghost" size="icon" onclick={() => handleEditPresetName(preset)}>
				<Edit class="h-4 w-4" />
			</Button>
		{/snippet}
		{#snippet listItemPopup(preset)}
			<div class="flex flex-col">
				<span class="text-lg font-bold">{preset.name}</span>
				<div class="flex justify-between">
					<span class="mr-2">{c.items}:</span>
					<span>
						{preset.storage_container?.slots.filter((slot) => slot.static_id !== 'None').length ||
							0}
					</span>
				</div>
			</div>
		{/snippet}
	</List>
</div>
