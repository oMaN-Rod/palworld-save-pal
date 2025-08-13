<script lang="ts">
	import { TextInputModal } from '$components/modals';
	import { List, TooltipButton } from '$components/ui';
	import { itemsData, presetsData } from '$lib/data';
	import type { ItemContainer, ItemContainerSlot, PresetProfile } from '$lib/types';
	import { getModalState } from '$states';
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

	let { container, onUpdate } = $props<{
		container: ItemContainer & { slots: ItemContainerSlot[] };
		onUpdate: () => void;
	}>();

	const modal = getModalState();

	type ExtendedPresetProfile = PresetProfile & { id: string };

	let selectedPreset: ExtendedPresetProfile = $state({ id: '', name: '', type: 'storage' });
	let selectedPresets: ExtendedPresetProfile[] = $state([]);
	let selectAll: boolean = $state(false);

	let filteredPresets: ExtendedPresetProfile[] = $derived.by(() => {
		return Object.entries(presetsData.presetProfiles)
			.filter(
				([_, preset]) =>
					preset.type === 'storage' && preset.storage_container?.key === container.key
			)
			.map(([id, preset]) => ({ ...preset, id }));
	});

	async function handleApplyPreset() {
		if (!selectedPresets.length || !container) return;
		const preset = selectedPresets[0];
		if (!preset.storage_container) return;

		const updatedSlots = container.slots.map((slot: ItemContainerSlot) => {
			const presetSlot = preset.storage_container!.slots.find(
				(ps) => ps.slot_index === slot.slot_index
			);
			if (presetSlot) {
				return { ...slot, ...presetSlot };
			}
			return { ...slot, static_id: 'None', count: 0, dynamic_item: undefined };
		});

		container.slots = updatedSlots;
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
			title: 'Add storage preset',
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
			title: 'Delete presets',
			message: `Are you sure you want to delete ${selectedPresets.length} preset${selectedPresets.length > 1 ? 's' : ''}?`
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
			title: 'Edit preset name',
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
			title: 'Select Item'
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
			title: 'Enter Item Count',
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

<div class="flex min-w-64 max-w-96 flex-col space-y-2">
	<div class="btn-group bg-surface-900 items-center rounded-sm p-1">
		<TooltipButton onclick={handleAddPreset} popupLabel="Create a preset from current container">
			<Plus />
		</TooltipButton>
		<TooltipButton onclick={handleFillContainer} popupLabel="Fill current container">
			<PaintBucket />
		</TooltipButton>
		<TooltipButton onclick={handleSetContainerCount} popupLabel="Set container item count">
			<Hash />
		</TooltipButton>
		<TooltipButton onclick={handleClearContainer} popupLabel="Clear container">
			<ChevronsLeftRight />
		</TooltipButton>
		{#if selectedPresets.length === 1}
			<TooltipButton onclick={handleApplyPreset} popupLabel="Apply selected preset">
				<Play />
			</TooltipButton>
		{/if}
		{#if selectedPresets.length >= 1}
			<TooltipButton onclick={handleDeletePresets} popupLabel="Delete selected preset(s)">
				<Trash />
			</TooltipButton>
			<TooltipButton onclick={() => (selectedPresets = [])} popupLabel="Clear selected">
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
				<span class="font-bold">Preset</span>
			</div>
		{/snippet}
		{#snippet listItem(preset)}
			<span class="grow">{preset.name}</span>
		{/snippet}
		{#snippet listItemActions(preset)}
			<button class="btn" onclick={() => handleEditPresetName(preset)}>
				<Edit class="h-4 w-4" />
			</button>
		{/snippet}
		{#snippet listItemPopup(preset)}
			<div class="flex flex-col">
				<span class="text-lg font-bold">{preset.name}</span>
				<div class="flex justify-between">
					<span class="mr-2">Items:</span>
					<span>
						{preset.storage_container?.slots.filter((slot) => slot.static_id !== 'None').length ||
							0}
					</span>
				</div>
			</div>
		{/snippet}
	</List>
</div>
