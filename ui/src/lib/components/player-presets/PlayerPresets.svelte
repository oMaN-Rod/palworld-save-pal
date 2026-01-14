<script lang="ts">
	import { TextInputModal } from '$components/modals';
	import { List, TooltipButton } from '$components/ui';
	import { presetsData, itemsData } from '$lib/data';
	import type { ItemContainerSlot, Player, PresetProfile } from '$lib/types';
	import { getAppState, getModalState } from '$states';
	import { EntryState, ItemTypeA } from '$types';
	import { deepCopy } from '$utils';
	import { Edit, Play, Plus, Trash, X } from 'lucide-svelte';
	import * as m from '$i18n/messages';
	import { c } from '$utils/commonTranslations';
	
	let { containerRef, player = $bindable() } = $props<{
		containerRef: HTMLDivElement | null;
		player: Player | undefined;
	}>();

	const appState = getAppState();
	const modal = getModalState();

	type ExtendedPresetProfile = PresetProfile & { id: string };

	let selectedPreset: ExtendedPresetProfile = $state({ id: '', name: '', type: 'inventory' });
	let selectedPresets: ExtendedPresetProfile[] = $state([]);
	let selectAll: boolean = $state(false);
	let listWrapperStyle = $state('');

	let filteredPresets: ExtendedPresetProfile[] = $derived.by(() => {
		return Object.entries(presetsData.presetProfiles)
			.filter(([_, preset]) => preset.type === 'inventory')
			.map(([id, preset]) => ({ ...preset, id }));
	});

	function calculateHeight() {
		if (containerRef) {
			const rect = containerRef.getBoundingClientRect();
			const windowHeight = window.innerHeight;
			const listHeight = windowHeight - rect.top - 320;
			listWrapperStyle = `height: ${listHeight}px;`;
		}
	}

	async function handleApplyPreset() {
		if (!selectedPresets.length || !player) return;
		const containers = {
			common_container: player.common_container,
			essential_container: player.essential_container,
			weapon_load_out_container: player.weapon_load_out_container,
			player_equipment_armor_container: player.player_equipment_armor_container,
			food_equip_container: player.food_equip_container
		};

		for (const [containerName, container] of Object.entries(containers)) {
			if (!selectedPresets[0][containerName as keyof PresetProfile]) {
				continue;
			}

			const presetSlots =
				selectedPresets[0][containerName as keyof PresetProfile] || container.slots;

			for (let i = 0; i < container.slots.length; i++) {
				const slot = container.slots[i];
				const presetSlot = (presetSlots as ItemContainerSlot[])?.find(
					(ps) => ps.slot_index === slot.slot_index
				);

				if (presetSlot) {
					slot.static_id = presetSlot.static_id;
					slot.count = presetSlot.count;
					slot.dynamic_item = presetSlot.dynamic_item;
				} else {
					slot.static_id = 'None';
					slot.count = 0;
					slot.dynamic_item = undefined;
				}
			}

			for (const slot of container.slots) {
				if (slot.static_id !== 'None') {
					const itemData = itemsData.getByKey(slot.static_id);
					if (
						itemData &&
						(itemData.details.type_a === ItemTypeA.Accessory || !itemData.details.dynamic)
					) {
						slot.dynamic_item = undefined;
					} else if (itemData?.details.dynamic) {
						switch (itemData.details.dynamic.type) {
							case 'weapon':
								slot.dynamic_item = {
									local_id: '00000000-0000-0000-0000-000000000000',
									durability: itemData.details.dynamic.durability || 0,
									remaining_bullets: itemData.details.dynamic.magazine_size || 0,
									type: itemData.details.dynamic.type
								};
								break;
							case 'armor':
								slot.dynamic_item = {
									local_id: '00000000-0000-0000-0000-000000000000',
									durability: itemData.details.dynamic.durability || 0,
									type: itemData.details.dynamic.type
								};
								break;
						}
					}
				}
			}
		}

		player.state = EntryState.MODIFIED;
	}

	function processSlots(slots: ItemContainerSlot[]) {
		const newSlots = deepCopy(slots);
		return newSlots
			.filter((s) => s.static_id !== 'None')
			.map((slot) => {
				if (slot.dynamic_item) {
					slot.dynamic_item.local_id = '00000000-0000-0000-0000-000000000000';
					return { ...slot };
				}
				return slot;
			});
	}

	async function handleAddPreset() {
		if (!player) return;
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: m.add_entity({entity: c.presets}),
			value: ''
		});
		if (!result) return;
		const newPreset = {
			name: result,
			type: 'inventory',
			common_container: processSlots(player.common_container.slots),
			essential_container: processSlots(player.essential_container.slots),
			weapon_load_out_container: processSlots(player.weapon_load_out_container.slots),
			player_equipment_armor_container: processSlots(player.player_equipment_armor_container.slots),
			food_equip_container: processSlots(player.food_equip_container.slots)
		} as PresetProfile;
		await presetsData.addPresetProfile(newPreset);
		selectedPresets = [];
		selectAll = false;
	}

	async function handleDeletePresets() {
		if (selectedPresets.length === 0) return;
		// @ts-ignore
		const result = await modal.showConfirmModal({
			title: m.delete_entity({entity: c.presets}),
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
			title: m.edit_entity({entity: c.preset}),
			value: preset.name
		});
		if (!result) return;
		await presetsData.changePresetName(preset.id, result);
	}

	$effect(() => {
		calculateHeight();
	});

	$effect(() => {
		window.addEventListener('resize', calculateHeight);
		return () => {
			window.removeEventListener('resize', calculateHeight);
		};
	});
</script>

<div class="flex min-w-64 max-w-96 flex-col space-y-2">
	<div class="btn-group bg-surface-900 items-center rounded-sm p-1">
		<TooltipButton
			onclick={handleAddPreset}
			popupLabel={m.create_preset_from_current({ entity: m.inventory() })}
		>
			<Plus />
		</TooltipButton>
		{#if selectedPresets.length === 1}
			<TooltipButton onclick={handleApplyPreset} popupLabel={m.apply_selected_entity({ entity: c.preset })}>
				<Play />
			</TooltipButton>
		{/if}
		{#if selectedPresets.length >= 1}
			<TooltipButton onclick={handleDeletePresets} popupLabel={m.delete_selected_entity({ entity: m.preset({ count: selectedPresets.length }) })}>
				<Trash />
			</TooltipButton>
			<TooltipButton onclick={() => (selectedPresets = [])} popupLabel={m.clear_selected()}>
				<X />
			</TooltipButton>
		{/if}
	</div>
	<div class="overflow-y-scroll" style={listWrapperStyle}>
		<List
			baseClass="bg-surface-800"
			listClass="overflow-y-scroll"
			items={filteredPresets}
			bind:selectedItems={selectedPresets}
			bind:selectedItem={selectedPreset}
			bind:selectAll
			onlyHighlightChecked
		>
			{#snippet listHeader()}
				<div class="flex justify-start">
					<span class="font-bold">{c.presets}</span>
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
						? `${preset.common_container.length} ${c.items}`
						: '💩'}
				{@const essentialContainerString =
					preset.essential_container && preset.essential_container.length > 0
						? `${preset.essential_container.length} ${c.items}`
						: '💩'}
				{@const weaponLoadOutContainerString =
					preset.weapon_load_out_container && preset.weapon_load_out_container.length > 0
						? `${preset.weapon_load_out_container.length} ${c.items}`
						: '💩'}
				{@const playerEquipmentArmorContainerString =
					preset.player_equipment_armor_container &&
					preset.player_equipment_armor_container.length > 0
						? `${preset.player_equipment_armor_container.length} ${c.items}`
						: '💩'}
				{@const foodEquipContainerString =
					preset.food_equip_container && preset.food_equip_container.length > 0
						? `${preset.food_equip_container.length} ${c.items}`
						: '💩'}
				<div class="flex flex-col">
					<span class="text-lg font-bold">{preset.name}</span>
					<div class="flex flex-col space-y-2">
						<div class="flex justify-between">
							<span class="mr-2">{m.container_entity({entity: m.common})}</span>
							<span>{commonContainerString}</span>
						</div>
						<div class="flex justify-between">
							<span class="mr-2">{m.container_entity({entity: m.essential()})}</span>
							<span>{essentialContainerString}</span>
						</div>
						<div class="flex justify-between">
							<span class="mr-2">{m.container_entity({ entity: m.weapon({count: 1})})}</span>
							<span>{weaponLoadOutContainerString}</span>
						</div>
						<div class="flex justify-between">
							<span class="mr-2">{m.container_entity({entity: m.armor()})}</span>
							<span>{playerEquipmentArmorContainerString}</span>
						</div>
						<div class="flex justify-between">
							<span class="mr-2">{m.container_entity({entity: m.food()})}</span>
							<span>{foodEquipContainerString}</span>
						</div>
					</div>
				</div>
			{/snippet}
		</List>
	</div>
</div>
