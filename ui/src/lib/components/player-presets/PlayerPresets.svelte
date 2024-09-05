<script lang="ts">
	import { TextInputModal } from '$components/modals';
	import { ItemHeader, List, Tooltip } from '$components/ui';
	import { presetsData, itemsData } from '$lib/data';
	import type { PresetProfile } from '$lib/data/presets';
	import { getAppState, getModalState } from '$states';
	import { EntryState } from '$types';
	import { deepCopy } from '$utils';
	import { Copy, Edit, Play, Plus, Trash, X } from 'lucide-svelte';

	const appState = getAppState();
	const modal = getModalState();

	let selectedPreset: PresetProfile = $state({ name: '' });
	let selectedPresets: PresetProfile[] = $state([]);
	let presets: PresetProfile[] = $state([]);

	async function getPresetProfiles() {
		presets = await presetsData.getPresetProfiles();
	}

	async function handleApplyPreset() {
		if (!selectedPreset || !appState.selectedPlayer) return;
		console.log('Applying preset:', selectedPreset);
		const containers = {
			common_container: appState.selectedPlayer.common_container,
			essential_container: appState.selectedPlayer.essential_container,
			weapon_load_out_container: appState.selectedPlayer.weapon_load_out_container,
			player_equipment_armor_container: appState.selectedPlayer.player_equipment_armor_container,
			food_equip_container: appState.selectedPlayer.food_equip_container
		};

		const updatedContainers = await presetsData.applyPreset(selectedPreset.name, containers);
		console.log('Updated containers:', updatedContainers);
		// Update dynamic items
		for (const [containerName, container] of Object.entries(updatedContainers)) {
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

	$effect(() => {
		getPresetProfiles();
	});

	async function handleAddPreset() {
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		const result = (await modal.showModal(TextInputModal, {
			title: 'Add preset',
			value: ''
		})) as string;
		if (result) {
			let newPreset = {
				name: result,
				common_container: deepCopy(appState.selectedPlayer.common_container.slots),
				essential_container: deepCopy(appState.selectedPlayer.essential_container.slots),
				weapon_load_out_container: deepCopy(
					appState.selectedPlayer.weapon_load_out_container.slots
				),
				player_equipment_armor_container: deepCopy(
					appState.selectedPlayer.player_equipment_armor_container.slots
				),
				food_equip_container: deepCopy(appState.selectedPlayer.food_equip_container.slots)
			};
			presets = await presetsData.addPresetProfile(newPreset);
		}
	}

	async function handleClonePreset() {
		if (!selectedPreset) return;
		// @ts-ignore
		const result = (await modal.showModal(TextInputModal, {
			title: 'Edit preset name',
			value: selectedPreset.name
		})) as string;
		if (result) {
			console.log('Cloning preset:', selectedPreset);
			presets = await presetsData.clone(selectedPreset.name, result);
		}
	}

	async function handleDeletePreset() {
		if (selectedPresets.length === 0) return;
		presets = await presetsData.removePresetProfiles(selectedPresets.map((preset) => preset.name));
	}

	async function handleEditPreset(preset: PresetProfile) {
		// @ts-ignore
		const result = (await modal.showModal(TextInputModal, {
			title: 'Edit preset name',
			value: preset.name
		})) as string;
		if (result) {
			console.log('Editing preset:', selectedPreset);
			presets = await presetsData.changeProfileName(preset.name, result);
		}
	}
</script>

<div class="mr-4 flex min-w-64 max-w-96 flex-col space-y-2">
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
			<Tooltip>
				<button class="btn hover:preset-tonal-secondary p-2" onclick={handleClonePreset}>
					<Copy />
				</button>
				{#snippet popup()}
					Clone selected preset
				{/snippet}
			</Tooltip>
		{/if}
		{#if selectedPresets.length >= 1}
			<Tooltip>
				<button class="btn hover:preset-tonal-secondary p-2" onclick={handleDeletePreset}>
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
	<List
		baseClass="bg-surface-800"
		itemsKey="name"
		bind:items={presets}
		bind:selectedItems={selectedPresets}
		bind:selectedItem={selectedPreset}
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
			<button class="btn" onclick={() => handleEditPreset(preset)}>
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
