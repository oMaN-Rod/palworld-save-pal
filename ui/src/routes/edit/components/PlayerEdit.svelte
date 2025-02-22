<script lang="ts">
	import { ItemHeader, Progress } from '$components/ui';
	import { getAppState, getToastState, getModalState } from '$states';
	import { EntryState, type ItemContainerSlot, type ItemContainer } from '$types';
	import { ASSET_DATA_PATH, MAX_LEVEL } from '$lib/constants';
	import { itemsData, expData } from '$lib/data';
	import { Tabs, Accordion } from '@skeletonlabs/skeleton-svelte';
	import { Tooltip } from '$components/ui';
	import {
		ItemBadge,
		PlayerPresets,
		PlayerStats,
		PlayerHealthBadge,
		TextInputModal
	} from '$components';
	import {
		Bomb,
		ChevronsLeftRight,
		Key,
		Pizza,
		Shield,
		Swords,
		ArrowUp01,
		Minus,
		Plus,
		Edit
	} from 'lucide-svelte';
	import { assetLoader } from '$utils';

	const appState = getAppState();
	const toast = getToastState();
	const modal = getModalState();

	const defaultItem = {
		id: '',
		type: '',
		slots: [],
		key: '',
		slot_num: 0
	};
	const defaultItemContainerSlot = {
		id: '',
		static_id: '',
		slot_index: 0,
		type: '',
		count: 0
	};

	let commonContainer: ItemContainer = $state(defaultItem);
	let essentialContainer: ItemContainer = $state(defaultItem);
	let weaponLoadOutContainer: ItemContainer = $state(defaultItem);
	let playerEquipmentArmorContainer: ItemContainer = $state(defaultItem);
	let foodEquipContainer: ItemContainer = $state(defaultItem);
	let headGear: ItemContainerSlot = $state(defaultItemContainerSlot);
	let bodyGear: ItemContainerSlot = $state(defaultItemContainerSlot);
	let shieldGear: ItemContainerSlot = $state(defaultItemContainerSlot);
	let gliderGear: ItemContainerSlot = $state(defaultItemContainerSlot);
	let sphereModule: ItemContainerSlot = $state(defaultItemContainerSlot);
	let accessoryGear: ItemContainerSlot[] = $state([]);
	let group = $state('inventory');
	let sideBarExpanded: string[] = $state(['stats']);
	let sideBarWrapper: HTMLDivElement | null = $state(null);

	let health = $state(500);

	let foodSlotCount = $derived.by(() => {
		let slotCount = 0;
		Object.values(essentialContainer.slots).forEach((slot) => {
			if (slot.static_id.includes('AutoMealPouch_Tier')) {
				const foodCount = parseInt(slot.static_id.slice(-1));
				slotCount = foodCount > slotCount ? foodCount : slotCount;
			}
		});
		return slotCount;
	});

	let inventorySlotCount = $derived.by(() => {
		let extraSlots = 0;
		Object.values(essentialContainer.slots).forEach((slot) => {
			if (slot.static_id.includes('AdditionalInventory_')) {
				extraSlots += 3;
			}
		});
		return Math.min(42 + extraSlots, 54);
	});

	let { levelProgressToNext, levelProgressValue, levelProgressMax } = $derived.by(() => {
		if (appState.selectedPlayer) {
			if (appState.selectedPlayer.level === MAX_LEVEL) {
				return { levelProgressToNext: 0, levelProgressValue: 0, levelProgressMax: 1 };
			}
			const nextExp = expData.expData[appState.selectedPlayer.level + 1];
			return {
				levelProgressToNext: nextExp.TotalEXP - appState.selectedPlayer.exp,
				levelProgressValue: nextExp.NextEXP - (nextExp.TotalEXP - appState.selectedPlayer.exp),
				levelProgressMax: nextExp.NextEXP
			};
		}
		return { levelProgressToNext: 0, levelProgressValue: 0, levelProgressMax: 1 };
	});

	async function getItemIcon(staticId: string) {
		if (!staticId || staticId === 'None') return;
		const itemData = itemsData.items[staticId] || undefined;
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${itemData.details.icon}.png`);
	}

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

	function clearAll() {
		clearCommonContainer();
		clearEssentialContainer();
		clearWeaponLoadOutContainer();
		clearEquipmentArmorContainer();
		clearFoodEquipContainer();
	}

	async function copyItem(slot: ItemContainerSlot) {
		if (slot.static_id !== 'None') {
			appState.clipboardItem = slot;
			let itemName = slot.static_id;
			const itemData = itemsData.items[slot.static_id];
			if (itemData) {
				itemName = itemData.info.localized_name;
			}
			toast.add(`${itemName} copied to clipboard`);
		} else {
			appState.clipboardItem = null;
			toast.add('Clipboard cleared');
		}
	}

	function clearItem(slot: ItemContainerSlot) {
		slot.static_id = 'None';
		slot.count = 0;
		slot.dynamic_item = undefined;
		if (appState.selectedPlayer) {
			appState.selectedPlayer.state = EntryState.MODIFIED;
		}
	}

	function pasteItem(slot: ItemContainerSlot) {
		if (appState.clipboardItem) {
			slot.static_id = appState.clipboardItem.static_id;
			slot.count = appState.clipboardItem.count;
			slot.dynamic_item = appState.clipboardItem.dynamic_item;
			if (slot.dynamic_item) {
				slot.dynamic_item.local_id = '00000000-0000-0000-0000-000000000000';
			}
			if (appState.selectedPlayer) {
				appState.selectedPlayer.state = EntryState.MODIFIED;
			}
		} else {
			clearItem(slot);
		}
	}

	async function handleCopyPaste(event: MouseEvent, slot: ItemContainerSlot, canPaste = true) {
		if (event.button === 0) return;
		event.preventDefault();
		if (event.ctrlKey && event.button === 2 && canPaste) {
			pasteItem(slot);
		} else if (event.ctrlKey && event.button === 1) {
			clearItem(slot);
		} else if (!event.ctrlKey && event.button === 2) {
			await copyItem(slot);
		} else {
			toast.add('Cannot paste here (yet™)', undefined, 'warning');
		}
	}

	function onItemUpdate() {
		if (appState.selectedPlayer) {
			appState.selectedPlayer.state = EntryState.MODIFIED;
		}
	}

	function loadCommonContainer() {
		if (appState.selectedPlayer) {
			commonContainer.slots = [];
			const container = appState.selectedPlayer.common_container;
			container.slots.sort((a, b) => a.slot_index - b.slot_index);
			let containerSlots = [];
			for (let i = 0; i < inventorySlotCount; i++) {
				const slot = container.slots.find((s) => s.slot_index === i);
				if (!slot) {
					const emptySlot = {
						static_id: 'None',
						slot_index: i,
						count: 0,
						dynamic_item: undefined
					};
					containerSlots.push(emptySlot);
					appState.selectedPlayer.common_container.slots.push(emptySlot);
				} else {
					containerSlots.push(slot);
				}
			}
			commonContainer.slots = containerSlots;
		}
	}

	function loadEssentialContainer() {
		if (appState.selectedPlayer) {
			const container = appState.selectedPlayer.essential_container;
			container.slots.sort((a, b) => a.slot_index - b.slot_index);
			let containerSlots = [];
			for (let i = 0; i < 200; i++) {
				const slot = container.slots.find((s) => s.slot_index === i);
				if (!slot) {
					const emptySlot = {
						static_id: 'None',
						slot_index: i,
						count: 0,
						dynamic_item: undefined
					};
					containerSlots.push(emptySlot);
					appState.selectedPlayer.essential_container.slots.push(emptySlot);
				} else {
					containerSlots.push(slot);
				}
			}
			essentialContainer.slots = containerSlots;
		}
	}

	function loadFoodContainer() {
		if (appState.selectedPlayer) {
			const container = appState.selectedPlayer.food_equip_container;
			let containerSlots = [];
			for (let i = 0; i < foodSlotCount; i++) {
				const slot = container.slots.find((s) => s.slot_index === i);
				if (!slot) {
					const emptySlot = {
						static_id: 'None',
						slot_index: i,
						count: 0,
						dynamic_item: undefined
					};
					containerSlots.push(emptySlot);
					appState.selectedPlayer.food_equip_container.slots.push(emptySlot);
				} else {
					containerSlots.push(slot);
				}
			}
			foodEquipContainer.slots = containerSlots;
		}
	}

	function loadWeaponLoadoutContainer() {
		if (appState.selectedPlayer) {
			const container = appState.selectedPlayer.weapon_load_out_container;
			container.slots.sort((a, b) => a.slot_index - b.slot_index);
			let containerSlots = [];
			for (let i = 0; i < 4; i++) {
				const slot = container.slots.find((s) => s.slot_index === i);
				if (!slot) {
					const emptySlot = {
						static_id: 'None',
						slot_index: i,
						count: 0,
						dynamic_item: undefined
					};
					containerSlots.push(emptySlot);
					appState.selectedPlayer.weapon_load_out_container.slots.push(emptySlot);
				} else {
					containerSlots.push(slot);
				}
			}
			weaponLoadOutContainer.slots = containerSlots;
		}
	}

	function loadPlayerEquipmentArmorContainer() {
		if (appState.selectedPlayer) {
			const container = appState.selectedPlayer.player_equipment_armor_container;
			container.slots.sort((a, b) => a.slot_index - b.slot_index);
			let containerSlots = [];
			for (let i = 0; i < 9; i++) {
				const slot = container.slots.find((s) => s.slot_index === i);
				if (!slot) {
					const emptySlot = {
						static_id: 'None',
						slot_index: i,
						count: 0,
						dynamic_item: undefined
					};
					containerSlots.push(emptySlot);
					appState.selectedPlayer.player_equipment_armor_container.slots.push(emptySlot);
				} else {
					containerSlots.push(slot);
				}
			}

			headGear = containerSlots[0];
			bodyGear = containerSlots[1];
			shieldGear = containerSlots[4];
			gliderGear = containerSlots[5];
			sphereModule = containerSlots[8];
			accessoryGear = containerSlots.slice(2, 4).concat(containerSlots.slice(6, 8));
			playerEquipmentArmorContainer.slots = containerSlots;
		}
	}

	async function sortCommonContainer() {
		if (appState.selectedPlayer) {
			const sortedSlots = commonContainer.slots.map((slot) => {
				if (slot.static_id !== 'None') {
					const itemData = itemsData.items[slot.static_id];
					return { ...slot, sort_id: itemData?.details.sort_id ?? Infinity };
				}
				return { ...slot, sort_id: Infinity };
			});

			sortedSlots.sort((a, b) => a.sort_id - b.sort_id);

			commonContainer.slots = sortedSlots.map((slot, index) => ({
				...slot,
				slot_index: index
			}));

			appState.selectedPlayer.common_container.slots = commonContainer.slots;
			appState.selectedPlayer.state = EntryState.MODIFIED;
		}
	}

	async function handleLevelIncrement() {
		if (!appState.selectedPlayer || !appState.selectedPlayer || !appState.selectedPlayer.pals)
			return;

		const newLevel = Math.min(appState.selectedPlayer.level + 1, MAX_LEVEL);
		if (newLevel === appState.selectedPlayer.level) return;

		const nextLevelData = await expData.getExpDataByLevel(newLevel + 1);

		appState.selectedPlayer.level = newLevel;
		appState.selectedPlayer.exp = nextLevelData.TotalEXP - nextLevelData.NextEXP;
		appState.selectedPlayer.state = EntryState.MODIFIED;
	}

	async function handleLevelDecrement() {
		if (!appState.selectedPlayer || !appState.selectedPlayer || !appState.selectedPlayer.pals)
			return;

		const newLevel = Math.max(appState.selectedPlayer.level - 1, 1);
		if (newLevel === appState.selectedPlayer.level) return;

		const newLevelData = await expData.getExpDataByLevel(newLevel + 1);

		appState.selectedPlayer.level = newLevel;
		appState.selectedPlayer.exp = newLevelData.TotalEXP - newLevelData.NextEXP;
		appState.selectedPlayer.state = EntryState.MODIFIED;
	}

	$effect(() => {
		if (appState.selectedPlayer) {
			loadCommonContainer();
			loadEssentialContainer();
			loadFoodContainer();
			loadWeaponLoadoutContainer();
			loadPlayerEquipmentArmorContainer();
			health = 500 + appState.selectedPlayer.status_point_list.max_hp * 100;
		}
	});

	async function handleUpdateNickname() {
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: 'Change Player Name',
			value: appState.selectedPlayer.nickname
		});
		if (result) {
			appState.selectedPlayer.nickname = result;
			appState.selectedPlayer.state = EntryState.MODIFIED;
		}
	}
</script>

{#if appState.selectedPlayer}
	<div class="flex h-full flex-col overflow-auto">
		<div class="ml-2 flex">
			<!-- Main content wrapper -->
			<div class="grid w-full grid-cols-[auto_1fr] gap-4 pr-[420px]">
				<!-- Inventory -->
				<div class="flex h-[600px] flex-col">
					<div class="mb-4 flex items-center space-x-2">
						<Tooltip>
							<button
								class="btn preset-filled-primary-500 hover:preset-tonal-secondary xl:btn-md btn-sm px-2 xl:px-4"
								onclick={clearCommonContainer}
							>
								<ChevronsLeftRight class="h-4 w-4 xl:h-6 xl:w-6" />
							</button>
							{#snippet popup()}
								<span>Clear Inventory</span>
							{/snippet}
						</Tooltip>
						<Tooltip>
							<button
								class="btn preset-filled-primary-500 hover:preset-tonal-secondary btn-sm xl:btn-md px-2 xl:px-4"
								onclick={sortCommonContainer}
							>
								<ArrowUp01 class="h-4 w-4 xl:h-6 xl:w-6" />
							</button>
							{#snippet popup()}
								<span>Sort Inventory</span>
							{/snippet}
						</Tooltip>
						<Tooltip
							><button
								class="btn preset-filled-primary-500 hover:preset-tonal-secondary xl:btn-md btn-sm px-2 xl:px-4"
								onclick={clearEssentialContainer}
							>
								<Key class="h-4 w-4 xl:h-6 xl:w-6" />
							</button>
							{#snippet popup()}
								<span>Clear Key Items</span>
							{/snippet}
						</Tooltip>
						<Tooltip>
							<button
								class="btn preset-filled-primary-500 hover:preset-tonal-secondary xl:btn-md btn-sm px-2 xl:px-4"
								onclick={clearWeaponLoadOutContainer}
							>
								<Swords class="h-4 w-4 xl:h-6 xl:w-6" />
							</button>
							{#snippet popup()}
								<span>Clear Weapons</span>
							{/snippet}
						</Tooltip>
						<Tooltip>
							<button
								class="btn preset-filled-primary-500 hover:preset-tonal-secondary xl:btn-md btn-sm px-2 xl:px-4"
								onclick={clearEquipmentArmorContainer}
							>
								<Shield class="h-4 w-4 xl:h-6 xl:w-6" />
							</button>
							{#snippet popup()}
								<span>Clear Armor</span>
							{/snippet}
						</Tooltip>
						<Tooltip>
							<button
								class="btn preset-filled-primary-500 hover:preset-tonal-secondary xl:btn-md btn-sm px-2 xl:px-4"
								onclick={clearFoodEquipContainer}
							>
								<Pizza class="h-4 w-4 xl:h-6 xl:w-6" />
							</button>
							{#snippet popup()}
								<span>Clear Food</span>
							{/snippet}
						</Tooltip>
						<Tooltip>
							<button
								class="btn preset-filled-primary-500 hover:preset-tonal-secondary xl:btn-md btn-sm px-2 xl:px-4"
								onclick={clearAll}
							>
								<Bomb class="h-4 w-4 xl:h-6 xl:w-6" />
							</button>
							{#snippet popup()}
								<span>Clear All</span>
							{/snippet}
						</Tooltip>
					</div>
					<Tabs listBorder="border border-surface-800" listClasses="h-auto" bind:value={group}>
						{#snippet list()}
							<Tabs.Control
								value="inventory"
								classes="w-full"
								base="border-none hover:ring-secondary-500 hover:ring"
								labelBase="py-1"
								stateActive="bg-surface-700"
								padding="p-0"
							>
								Inventory
							</Tabs.Control>
							<Tabs.Control
								value="key_items"
								classes="w-full"
								base="border-none hover:ring-secondary-500 hover:ring"
								labelBase="py-1"
								stateActive="bg-surface-700"
								padding="p-0"
							>
								Key Items
							</Tabs.Control>
						{/snippet}
						{#snippet content()}
							<Tabs.Panel value="inventory">
								<div class="max-h-[500px] overflow-y-auto 2xl:max-h-[800px]">
									<div class="grid grid-cols-6 gap-2">
										{#each Object.values(commonContainer.slots) as _, index}
											<ItemBadge
												bind:slot={commonContainer.slots[index]}
												itemGroup="Common"
												onCopyPaste={(event) =>
													handleCopyPaste(event, commonContainer.slots[index])}
												onUpdate={onItemUpdate}
											/>
										{/each}
									</div>
								</div>
							</Tabs.Panel>
							<Tabs.Panel value="key_items">
								<div class="max-h-[500px] overflow-y-auto 2xl:max-h-[800px]">
									<div class="grid grid-cols-6 gap-2">
										{#each Object.values(essentialContainer.slots) as _, index}
											<ItemBadge
												bind:slot={essentialContainer.slots[index]}
												itemGroup="KeyItem"
												onUpdate={onItemUpdate}
												onCopyPaste={(event) =>
													handleCopyPaste(event, essentialContainer.slots[index], false)}
											/>
										{/each}
									</div>
								</div>
							</Tabs.Panel>
						{/snippet}
					</Tabs>
				</div>
				<!-- Player Equip -->
				<div class="grid h-[600px] grid-cols-[auto_1fr_auto]">
					<div class="flex flex-col space-y-2">
						<ItemHeader text="Weapon" />
						<div class="flex max-w-[65px] flex-col space-y-2">
							{#each Object.values(weaponLoadOutContainer.slots) as _, index}
								<ItemBadge
									bind:slot={weaponLoadOutContainer.slots[index]}
									itemGroup="Weapon"
									onCopyPaste={(event) =>
										handleCopyPaste(event, weaponLoadOutContainer.slots[index], false)}
									onUpdate={onItemUpdate}
								/>
							{/each}
						</div>
						<ItemHeader text="Accessory" />
						<div class="ml-2">
							<div class="grid max-h-36 max-w-36 grid-cols-2 gap-2">
								{#each accessoryGear as _, index}
									<ItemBadge
										bind:slot={accessoryGear[index]}
										itemGroup="Accessory"
										onCopyPaste={(event) => handleCopyPaste(event, accessoryGear[index], false)}
										onUpdate={onItemUpdate}
									/>
								{/each}
							</div>
						</div>
					</div>
					<div class="flex flex-col items-center justify-center">
						<span class="flex h-1/3 items-end">
							{#await getItemIcon(headGear.static_id) then icon}
								{#if icon}
									<img
										src={icon}
										alt={headGear.static_id}
										class="hidden 2xl:block 2xl:h-16 2xl:w-16"
									/>
								{/if}
							{/await}
						</span>
						<span class="h-2/3">
							{#await getItemIcon(bodyGear.static_id) then icon}
								{#if icon}
									<img
										src={icon}
										alt={bodyGear.static_id}
										class="hidden 2xl:block 2xl:h-64 2xl:w-64"
									/>
								{/if}
							{/await}
						</span>
					</div>
					<div class="flex flex-col space-y-2">
						<ItemHeader text="Head" />
						<ItemBadge
							bind:slot={headGear}
							itemGroup="Head"
							onCopyPaste={(event) => handleCopyPaste(event, headGear, false)}
							onUpdate={onItemUpdate}
						/>
						<ItemHeader text="Body" />
						<ItemBadge
							bind:slot={bodyGear}
							itemGroup="Body"
							onCopyPaste={(event) => handleCopyPaste(event, bodyGear, false)}
							onUpdate={onItemUpdate}
						/>
						<ItemHeader text="Shield" />
						<ItemBadge
							bind:slot={shieldGear}
							itemGroup="Shield"
							onCopyPaste={(event) => handleCopyPaste(event, shieldGear, false)}
							onUpdate={onItemUpdate}
						/>
						<ItemHeader text="Glider" />
						<ItemBadge
							bind:slot={gliderGear}
							itemGroup="Glider"
							onCopyPaste={(event) => handleCopyPaste(event, gliderGear, false)}
							onUpdate={onItemUpdate}
						/>
						<ItemHeader text="Sphere Module" />
						<ItemBadge
							bind:slot={sphereModule}
							itemGroup="SphereModule"
							onCopyPaste={(event) => handleCopyPaste(event, sphereModule, false)}
							onUpdate={onItemUpdate}
						/>
					</div>
					<div class="col-span-3 ml-12 mt-2 space-y-2">
						<ItemHeader text="Food" />
						<div class="ml-2">
							<div class="flex flex-row space-x-2">
								{#each Object.values(foodEquipContainer.slots) as _, index}
									<ItemBadge
										bind:slot={foodEquipContainer.slots[index]}
										itemGroup="Food"
										onCopyPaste={(event) =>
											handleCopyPaste(event, foodEquipContainer.slots[index], false)}
										onUpdate={onItemUpdate}
									/>
								{/each}
							</div>
						</div>
					</div>
				</div>
			</div>

			<!-- Stats -->
			<div class="fixed right-2 w-96 flex-none" bind:this={sideBarWrapper}>
				<div
					class="border-l-surface-600 preset-filled-surface-100-900 mb-2 mr-2 flex rounded-none border-l-2 p-4"
				>
					<div class="mr-4 flex flex-col items-center justify-center rounded-none">
						<div class="flex px-2">
							<button class="mr-4">
								<Minus class="text-primary-500" size={16} onclick={handleLevelDecrement} />
							</button>

							<div class="flex flex-col items-center justify-center">
								<span class="text-surface-400 text-sm font-bold">LEVEL</span>
								<span class="text-xl font-bold xl:text-2xl">{appState.selectedPlayer.level}</span>
							</div>

							<button class="ml-4">
								<Plus class="text-primary-500" size={16} onclick={handleLevelIncrement} />
							</button>
						</div>
					</div>

					<div class="grow">
						<div class="flex flex-col">
							<div class="flex space-x-2">
								<button
									class="hover:ring-secondary-500 hover:ring-offset-surface-900 text-start font-bold hover:ring hover:ring-offset-4"
									onclick={handleUpdateNickname}
								>
									<Edit class="h-4 w-4" />
								</button>
								<span>{appState.selectedPlayer.nickname}</span>
							</div>
							<div class="flex flex-col space-y-2">
								<div class="flex">
									<span class="text-on-surface grow">NEXT</span>
									<span class="text-on-surface">{levelProgressToNext}</span>
								</div>
								<Progress
									value={levelProgressValue}
									max={levelProgressMax}
									height="h-2"
									width="w-full"
									rounded="rounded-none"
									showLabel={false}
								/>
							</div>
						</div>
					</div>
				</div>
				<PlayerHealthBadge bind:player={appState.selectedPlayer} bind:maxHp={health} />
				<Accordion value={sideBarExpanded} collapsible>
					<Accordion.Item value="stats">
						{#snippet control()}
							Stats
						{/snippet}
						{#snippet panel()}
							<PlayerStats player={appState.selectedPlayer!} />
						{/snippet}
					</Accordion.Item>
					<hr class="hr" />
					<Accordion.Item value="presets">
						{#snippet control()}Presets{/snippet}
						{#snippet panel()}
							<PlayerPresets containerRef={sideBarWrapper} />
						{/snippet}
					</Accordion.Item>
				</Accordion>
			</div>
		</div>
	</div>
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">Select a Player to edit 🚀</h2>
	</div>
{/if}

<style lang="postcss">
	img {
		opacity: 0;
		animation: fadeIn 0.3s ease-in forwards;
	}

	@keyframes fadeIn {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	img:not([src]) {
		animation: fadeOut 0.3s ease-out forwards;
	}

	@keyframes fadeOut {
		from {
			opacity: 1;
		}
		to {
			opacity: 0;
		}
	}
</style>
