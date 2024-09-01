<script lang="ts">
	import { Combobox, ItemHeader } from '$components/ui';
	import { getAppState } from '$states';
	import { EntryState, type ContainerSlot, type ItemContainer, type SelectOption } from '$types';
	import { assetLoader } from '$utils/asset-loader';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { itemsData, presetsData } from '$lib/data';
	import { Tabs } from '@skeletonlabs/skeleton-svelte';
	import { Tooltip } from '$components/ui';
	import { ItemBadge } from '$components';
	import { Bomb, Check, ChevronsLeftRight, Key, Pizza, Shield, Swords } from 'lucide-svelte';

	const appState = getAppState();

	let commonContainer: ItemContainer = $state({ id: '', type: '', slots: [] });
	let essentialContainer: ItemContainer = $state({ id: '', type: '', slots: [] });
	let weaponLoadOutContainer: ItemContainer = $state({ id: '', type: '', slots: [] });
	let playerEquipmentArmorContainer: ItemContainer = $state({ id: '', type: '', slots: [] });
	let foodEquipContainer: ItemContainer = $state({ id: '', type: '', slots: [] });
	let headGear: ContainerSlot = $state({
		id: '',
		static_id: '',
		slot_index: 0,
		type: '',
		count: 0
	});
	let bodyGear: ContainerSlot = $state({
		id: '',
		static_id: '',
		slot_index: 0,
		type: '',
		count: 0
	});
	let shieldGear: ContainerSlot = $state({
		id: '',
		static_id: '',
		slot_index: 0,
		type: '',
		count: 0
	});
	let gliderGear: ContainerSlot = $state({
		id: '',
		static_id: '',
		slot_index: 0,
		type: '',
		count: 0
	});
	let accessoryGear: ContainerSlot[] = $state([]);
	let group = $state('inventory');

	let presetOptions: SelectOption[] = $state([]);
	let selectedPreset = $state('');

	async function getItemIcon(staticId: string) {
		if (!staticId) return;
		const itemData = await itemsData.searchItems(staticId);
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		const iconPath = `${ASSET_DATA_PATH}/img/icons/${itemData.details.image}.png`;
		const icon = await assetLoader.loadImage(iconPath);
		return icon;
	}

	$effect(() => {
		if (appState.selectedPlayer) {
			commonContainer = appState.selectedPlayer.common_container;
			weaponLoadOutContainer = appState.selectedPlayer.weapon_load_out_container;
			playerEquipmentArmorContainer = appState.selectedPlayer.player_equipment_armor_container;
			headGear = playerEquipmentArmorContainer.slots[0];
			bodyGear = playerEquipmentArmorContainer.slots[1];
			shieldGear = playerEquipmentArmorContainer.slots[4];
			gliderGear = playerEquipmentArmorContainer.slots[5];
			accessoryGear = playerEquipmentArmorContainer.slots
				.slice(2, 4)
				.concat(playerEquipmentArmorContainer.slots.slice(6, 8));
			foodEquipContainer = appState.selectedPlayer.food_equip_container;
			essentialContainer = appState.selectedPlayer.essential_container;
		}
	});

	async function getPresetProfiles() {
		const profiles = await presetsData.getPresetProfiles();
		presetOptions = profiles.map((profile) => ({ label: profile, value: profile }));
	}

	async function applyPreset() {
		if (!selectedPreset || !appState.selectedPlayer) return;
		console.log('Applying preset:', selectedPreset);
		const containers = {
			common_container: appState.selectedPlayer.common_container,
			essential_container: appState.selectedPlayer.essential_container,
			weapon_load_out_container: appState.selectedPlayer.weapon_load_out_container,
			player_equipment_armor_container: appState.selectedPlayer.player_equipment_armor_container,
			food_equip_container: appState.selectedPlayer.food_equip_container
		};

		const updatedContainers = await presetsData.applyPreset(selectedPreset, containers);
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

	function clearCommonContainer() {
		Object.values(commonContainer.slots).forEach((slot) => {
			slot.dynamic_item = undefined;
			slot.static_id = 'None';
			slot.count = 0;
		});
		if (appState.selectedPlayer) {
			appState.selectedPlayer.state = EntryState.MODIFIED;
		}
	}

	function clearEssentialContainer() {
		Object.values(essentialContainer.slots).forEach((slot) => {
			slot.dynamic_item = undefined;
			slot.static_id = 'None';
			slot.count = 0;
		});
		if (appState.selectedPlayer) {
			appState.selectedPlayer.state = EntryState.MODIFIED;
		}
	}

	function clearWeaponLoadOutContainer() {
		Object.values(weaponLoadOutContainer.slots).forEach((slot) => {
			slot.dynamic_item = undefined;
			slot.static_id = 'None';
			slot.count = 0;
		});
		if (appState.selectedPlayer) {
			appState.selectedPlayer.state = EntryState.MODIFIED;
		}
	}

	function clearEquipmentArmorContainer() {
		Object.values(playerEquipmentArmorContainer.slots).forEach((slot) => {
			slot.dynamic_item = undefined;
			slot.static_id = 'None';
			slot.count = 0;
		});
		if (appState.selectedPlayer) {
			appState.selectedPlayer.state = EntryState.MODIFIED;
		}
	}

	function clearFoodEquipContainer() {
		Object.values(foodEquipContainer.slots).forEach((slot) => {
			slot.dynamic_item = undefined;
			slot.static_id = 'None';
			slot.count = 0;
		});
		if (appState.selectedPlayer) {
			appState.selectedPlayer.state = EntryState.MODIFIED;
		}
	}

	function clearAll(event: MouseEvent & { currentTarget: EventTarget & HTMLButtonElement }) {
		clearCommonContainer();
		clearEssentialContainer();
		clearWeaponLoadOutContainer();
		clearEquipmentArmorContainer();
		clearFoodEquipContainer();
	}
</script>

{#if appState.selectedPlayer}
	<div class="flex flex-col">
		<div class="m-2 flex flex-row items-center space-x-2 p-2">
			<h6 class="h6">Clear</h6>
			<Tooltip>
				<button
					class="btn preset-filled-primary-500 hover:preset-tonal-secondary"
					onclick={clearCommonContainer}
				>
					<ChevronsLeftRight />
				</button>
				{#snippet popup()}
					<span>Clear Inventory</span>
				{/snippet}
			</Tooltip>
			<Tooltip
				><button
					class="btn preset-filled-primary-500 hover:preset-tonal-secondary"
					onclick={clearEssentialContainer}
				>
					<Key />
				</button>
				{#snippet popup()}
					<span>Clear Key Items</span>
				{/snippet}
			</Tooltip>
			<Tooltip>
				<button
					class="btn preset-filled-primary-500 hover:preset-tonal-secondary"
					onclick={clearWeaponLoadOutContainer}
				>
					<Swords />
				</button>
				{#snippet popup()}
					<span>Clear Weapons</span>
				{/snippet}
			</Tooltip>
			<Tooltip>
				<button
					class="btn preset-filled-primary-500 hover:preset-tonal-secondary"
					onclick={clearEquipmentArmorContainer}
				>
					<Shield />
				</button>
				{#snippet popup()}
					<span>Clear Armor</span>
				{/snippet}
			</Tooltip>
			<Tooltip>
				<button
					class="btn preset-filled-primary-500 hover:preset-tonal-secondary"
					onclick={clearFoodEquipContainer}
				>
					<Pizza />
				</button>
				{#snippet popup()}
					<span>Clear Food</span>
				{/snippet}
			</Tooltip>
			<Tooltip>
				<button
					class="btn preset-filled-primary-500 hover:preset-tonal-secondary"
					onclick={clearAll}
				>
					<Bomb />
				</button>
				{#snippet popup()}
					<span>Clear All</span>
				{/snippet}
			</Tooltip>
			<h6 class="h6">Presets</h6>
			<div class="flex flex-row items-center space-x-2">
				<Combobox
					selectClass="bg-surface-900 max-w-54"
					options={presetOptions}
					bind:value={selectedPreset}
					placeholder="Select a preset"
				/>
				<Tooltip>
					<button
						class="btn preset-filled-primary-500 hover:preset-tonal-secondary"
						onclick={applyPreset}
					>
						<Check />
					</button>
					{#snippet popup()}
						<span>Apply Preset</span>
					{/snippet}
				</Tooltip>
			</div>
		</div>
		<div class="ml-2 grid grid-cols-[auto_1fr] gap-4">
			<Tabs listBorder="border border-surface-800" listClasses="h-auto">
				{#snippet list()}
					<Tabs.Control
						bind:group
						name="inventory"
						classes="w-full"
						border="border-none"
						active="bg-surface-700"
						contentBg="group-hover:font-bold"
						contentPadding="p-0"
						padding="p-0"
					>
						Inventory
					</Tabs.Control>
					<Tabs.Control
						bind:group
						name="key_items"
						classes="w-full"
						border="border-none"
						active="bg-surface-700"
						contentBg="group-hover:font-bold"
						contentPadding="p-0"
						padding="p-0"
					>
						Key Items
					</Tabs.Control>
				{/snippet}
				{#snippet panels()}
					<Tabs.Panel bind:group value="inventory">
						<div class="grid grid-cols-6 gap-2">
							{#each Object.values(commonContainer.slots) as _, index}
								<ItemBadge bind:slot={commonContainer.slots[index]} itemGroup="Common" />
							{/each}
						</div>
					</Tabs.Panel>
					<Tabs.Panel bind:group value="key_items">
						<div class="max-h-[500px] overflow-auto">
							<div class="grid grid-cols-6 gap-2">
								{#each Object.values(essentialContainer.slots) as _, index}
									<ItemBadge bind:slot={essentialContainer.slots[index]} itemGroup="KeyItem" />
								{/each}
							</div>
						</div>
					</Tabs.Panel>
				{/snippet}
			</Tabs>
			<div class="grid grid-cols-[auto_1fr_auto]">
				<div class="flex flex-col space-y-2">
					<ItemHeader text="Weapon" />
					<div class="flex max-w-[65px] flex-col space-y-2">
						{#each Object.values(weaponLoadOutContainer.slots) as _, index}
							<ItemBadge bind:slot={weaponLoadOutContainer.slots[index]} itemGroup="Weapon" />
						{/each}
					</div>
					<ItemHeader text="Accessory" />
					<div class="ml-2">
						<div class="grid max-h-36 max-w-36 grid-cols-2 gap-2">
							{#each accessoryGear as _, index}
								<ItemBadge bind:slot={accessoryGear[index]} itemGroup="Accessory" />
							{/each}
						</div>
					</div>
				</div>
				<div class="flex flex-col items-center justify-center">
					<span class="flex h-1/3 items-end">
						{#await getItemIcon(headGear.static_id) then icon}
							{#if icon}
								<enhanced:img src={icon} alt={headGear.static_id} style="width: 64px; height: 64px;"
								></enhanced:img>
							{/if}
						{/await}
					</span>
					<span class="h-2/3">
						{#await getItemIcon(bodyGear.static_id) then icon}
							{#if icon}
								<enhanced:img
									src={icon}
									alt={bodyGear.static_id}
									style="width: 256px; height: 256px;"
								></enhanced:img>
							{/if}
						{/await}
					</span>
				</div>
				<div class="mr-24 flex flex-col space-y-2">
					<ItemHeader text="Head" />
					<ItemBadge bind:slot={headGear} itemGroup="Head" />
					<ItemHeader text="Body" />
					<ItemBadge bind:slot={bodyGear} itemGroup="Body" />
					<ItemHeader text="Shield" />
					<ItemBadge bind:slot={shieldGear} itemGroup="Shield" />
					<ItemHeader text="Glider" />
					<ItemBadge bind:slot={gliderGear} itemGroup="Glider" />
				</div>
				<div class="col-span-3 ml-12 mt-4 space-y-2">
					<ItemHeader text="Food" />
					<div class="ml-2">
						<div class="flex flex-row space-x-2">
							{#each Object.values(foodEquipContainer.slots) as _, index}
								<ItemBadge bind:slot={foodEquipContainer.slots[index]} itemGroup="Food" />
							{/each}
						</div>
					</div>
				</div>
			</div>
		</div>
	</div>
{/if}
