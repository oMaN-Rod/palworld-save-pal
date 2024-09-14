<script lang="ts">
	import { TextInputModal } from '$components/modals';
	import { List, Tooltip } from '$components/ui';
	import { presetsData, itemsData } from '$lib/data';
	import type { ItemContainer, ItemContainerSlot, PresetProfile } from '$lib/types';
	import { getAppState, getModalState } from '$states';
	import { EntryState } from '$types';
	import { deepCopy } from '$utils';
	import { Copy, Edit, Play, Plus, Trash, X } from 'lucide-svelte';

	const appState = getAppState();
	const modal = getModalState();

	let selectedPreset: PresetProfile = $state({ name: '' });
	let selectedPresets: PresetProfile[] = $state([]);
	let presets: Record<string, PresetProfile> = $state({});
	let filteredPresets: PresetProfile[] = $state([]);
	let containerRef: HTMLDivElement;
	let listWrapperStyle = $state('');
	let selectAll: boolean = $state(false);

	function calculateHeight() {
		if (containerRef) {
			const rect = containerRef.getBoundingClientRect();
			const windowHeight = window.innerHeight;
			const listHeight = windowHeight - rect.top - 60;
			listWrapperStyle = `height: ${listHeight}px`;
		}
	}

	async function getPresetProfiles() {
		presets = await presetsData.getPresetProfiles();
		filteredPresets = Object.values(presets);
	}

	async function handleApplyPreset() {
		if (!selectedPresets || !appState.selectedPlayer) return;
		const containers = {
			common_container: appState.selectedPlayer.common_container,
			essential_container: appState.selectedPlayer.essential_container,
			weapon_load_out_container: appState.selectedPlayer.weapon_load_out_container,
			player_equipment_armor_container: appState.selectedPlayer.player_equipment_armor_container,
			food_equip_container: appState.selectedPlayer.food_equip_container
		};

		const updatedContainers: Record<string, ItemContainer> = {};

		for (const [containerName, container] of Object.entries(containers)) {
			const presetSlots =
				selectedPresets[0][containerName as keyof PresetProfile] || container.slots;
			const updatedSlots = container.slots.map((slot) => {
				const presetSlot = (presetSlots as ItemContainerSlot[])?.find(
					(ps) => ps.slot_index === slot.slot_index
				);
				if (presetSlot) {
					return { ...slot, ...presetSlot };
				}
				return { ...slot, static_id: 'None', count: 0, dynamic_item: undefined };
			});

			updatedContainers[containerName] = { ...container, slots: updatedSlots };
		}

		for (const [_, container] of Object.entries(updatedContainers)) {
			for (const slot of container.slots) {
				if (slot.static_id !== 'None') {
					const itemData = await itemsData.searchItems(slot.static_id);
					if (itemData?.details.dynamic) {
						switch (itemData.details.dynamic.type) {
							case 'weapon':
								slot.dynamic_item = {
									local_id: '00000000-0000-0000-0000-000000000000',
									durability: itemData.details.dynamic.durability,
									remaining_bullets: itemData.details.dynamic.magazine_size,
									type: itemData.details.dynamic.type
								};
								break;
							case 'armor':
								slot.dynamic_item = {
									local_id: '00000000-0000-0000-0000-000000000000',
									durability: itemData.details.dynamic.durability,
									type: itemData.details.dynamic.type
								};
								break;
						}
					} else {
						slot.dynamic_item = undefined;
					}
				}
			}
		}
		appState.selectedPlayer.state = EntryState.MODIFIED;
		appState.selectedPlayer = {
			...appState.selectedPlayer,
			...updatedContainers
		};
	}

	function processSlots(slots: ItemContainerSlot[]) {
		const newSlots = deepCopy(slots);
		return newSlots.map((slot) => {
			if (slot.dynamic_item) {
				slot.dynamic_item.local_id = '00000000-0000-0000-0000-000000000000';
				return { ...slot };
			}
			return slot;
		});
	}

	async function handleAddPreset() {
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: 'Add preset',
			value: ''
		});
		if (!result) return;
		let newPreset = {
			name: result,
			common_container: processSlots(appState.selectedPlayer.common_container.slots),
			essential_container: processSlots(appState.selectedPlayer.essential_container.slots),
			weapon_load_out_container: processSlots(
				appState.selectedPlayer.weapon_load_out_container.slots
			),
			player_equipment_armor_container: processSlots(
				appState.selectedPlayer.player_equipment_armor_container.slots
			),
			food_equip_container: processSlots(appState.selectedPlayer.food_equip_container.slots)
		};
		presets = await presetsData.addPresetProfile(newPreset);
		filteredPresets = Object.values(presets);
		selectedPresets = [];
		selectAll = false;
	}

	async function handleDeletePresets() {
		if (selectedPresets.length === 0) return;
		// @ts-ignore
		const result = await modal.showConfirmModal({
			title: 'Delete presets',
			message: `Are you sure you want to delete ${selectedPresets.length} preset${selectedPresets.length > 0 ? 's' : ''}?`
		});
		if (!result) return;
		const presetIds = selectedPresets
			.map((preset) => Object.keys(presets).find((key) => presets[key] === preset))
			.filter((id) => id !== undefined) as string[];
		if (!presetIds || presetIds.length === 0) return;
		presets = await presetsData.removePresetProfiles(presetIds);
		filteredPresets = Object.values(presets);
		selectedPresets = [];
		selectAll = false;
	}

	async function handleEditPresetName(preset: PresetProfile) {
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: 'Edit preset name',
			value: preset.name
		});
		if (!result) return;
		const presetId = Object.keys(presets).find((key) => presets[key] === preset);
		if (!presetId) return;
		presets = await presetsData.changePresetName(presetId, result);
		filteredPresets = Object.values(presets);
	}

	$effect(() => {
		getPresetProfiles();
		calculateHeight();
	});

	$effect(() => {
		window.addEventListener('resize', calculateHeight);
		return () => {
			window.removeEventListener('resize', calculateHeight);
		};
	});
</script>

<div class="mr-4 flex min-w-64 max-w-96 flex-col space-y-2" bind:this={containerRef}>
	<div class="btn-group bg-surface-900 items-center rounded p-1">
		<Tooltip position="left">
			<button class="btn hover:preset-tonal-secondary p-2" onclick={handleAddPreset}>
				<Plus />
			</button>
			{#snippet popup()}
				Create a preset from your current selection
			{/snippet}
		</Tooltip>
		{#if selectedPresets.length === 1}
			<Tooltip>
				<button class="btn hover:preset-tonal-secondary p-2" onclick={handleApplyPreset}>
					<Play />
				</button>
				{#snippet popup()}
					Apply selected preset
				{/snippet}
			</Tooltip>
		{/if}
		{#if selectedPresets.length >= 1}
			<Tooltip>
				<button class="btn hover:preset-tonal-secondary p-2" onclick={handleDeletePresets}>
					<Trash />
				</button>
				{#snippet popup()}
					Delete selected preset(s)
				{/snippet}
			</Tooltip>
			<Tooltip>
				<button class="btn hover:preset-tonal-secondary p-2" onclick={() => (selectedPresets = [])}>
					<X />
				</button>
				{#snippet popup()}
					Clear selected
				{/snippet}
			</Tooltip>
		{/if}
	</div>
	<div class="overflow-hidden" style={listWrapperStyle}>
		<List
			baseClass="h-full bg-surface-800"
			bind:items={filteredPresets}
			bind:selectedItems={selectedPresets}
			bind:selectedItem={selectedPreset}
			bind:selectAll
			onlyHighlightChecked
		>
			{#snippet listHeader()}
				<div class="flex justify-start">
					<span class="font-bold">Presets</span>
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
				{@const commonContainerString =
					preset.common_container && preset.common_container.length > 0
						? `${preset.common_container.length} items`
						: '💩'}
				{@const essentialContainerString =
					preset.essential_container && preset.essential_container.length > 0
						? `${preset.essential_container.length} items`
						: '💩'}
				{@const weaponLoadOutContainerString =
					preset.weapon_load_out_container && preset.weapon_load_out_container.length > 0
						? `${preset.weapon_load_out_container.length} items`
						: '💩'}
				{@const playerEquipmentArmorContainerString =
					preset.player_equipment_armor_container &&
					preset.player_equipment_armor_container.length > 0
						? `${preset.player_equipment_armor_container.length} items`
						: '💩'}
				{@const foodEquipContainerString =
					preset.food_equip_container && preset.food_equip_container.length > 0
						? `${preset.food_equip_container.length} items`
						: '💩'}
				<div class="flex flex-col">
					<span class="text-lg font-bold">{preset.name}</span>
					<div class="flex flex-col space-y-2">
						<div class="flex justify-between">
							<span class="mr-2">Common container:</span>
							<span>{commonContainerString}</span>
						</div>
						<div class="flex justify-between">
							<span class="mr-2">Essential container:</span>
							<span>{essentialContainerString}</span>
						</div>
						<div class="flex justify-between">
							<span class="mr-2">Weapon load out container:</span>
							<span>{weaponLoadOutContainerString}</span>
						</div>
						<div class="flex justify-between">
							<span class="mr-2">Player equipment armor container:</span>
							<span>{playerEquipmentArmorContainerString}</span>
						</div>
						<div class="flex justify-between">
							<span class="mr-2">Food equip container:</span>
							<span>{foodEquipContainerString}</span>
						</div>
					</div>
				</div>
			{/snippet}
		</List>
	</div>
</div>
