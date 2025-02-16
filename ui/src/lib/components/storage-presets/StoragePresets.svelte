<script lang="ts">
	import { TextInputModal } from '$components/modals';
	import { List, TooltipButton } from '$components/ui';
	import { presetsData } from '$lib/data';
	import type { ItemContainer, ItemContainerSlot, PresetProfile } from '$lib/types';
	import { getModalState } from '$states';
	import { EntryState } from '$types';
	import { deepCopy } from '$utils';
	import { Edit, Play, Plus, Trash, X } from 'lucide-svelte';

	let { container, onUpdate } = $props<{
		container: ItemContainer & { slots: ItemContainerSlot[] };
		onUpdate: () => void;
	}>();
	$inspect(container);
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
	$inspect(filteredPresets);

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
</script>

<div class="flex min-w-64 max-w-96 flex-col space-y-2">
	<div class="btn-group bg-surface-900 items-center rounded p-1">
		<TooltipButton onclick={handleAddPreset} popupLabel="Create a preset from current container">
			<Plus />
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
