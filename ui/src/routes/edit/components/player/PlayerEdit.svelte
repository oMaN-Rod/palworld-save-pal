<script lang="ts">
	import { ItemHeader, Progress } from '$components/ui';
	import { getAppState, getToastState, getModalState } from '$states';
	import { EntryState, type ItemContainerSlot, type ItemContainer } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { itemsData, expData } from '$lib/data';
	import { Tabs, Accordion } from '@skeletonlabs/skeleton-svelte';
	import { Tooltip } from '$components/ui';
	import {
		ItemBadge,
		PlayerPresets,
		PlayerStats,
		PlayerHealthBadge,
		TextInputModal,
		NumberInputModal,
		ItemSelectModal
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
		Edit,
		Hash,
		PaintBucket,
		PawPrint,
		Activity
	} from 'lucide-svelte';
	import { assetLoader } from '$utils';
	import { staticIcons } from '$types/icons';
	import NumberFlow from '@number-flow/svelte';

	const appState = getAppState();
	const toast = getToastState();
	const modal = getModalState();

	const max_level = $derived(appState.settings.cheat_mode ? 99 : 65);

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
			if (appState.selectedPlayer.level >= max_level) {
				return { levelProgressToNext: 0, levelProgressValue: 0, levelProgressMax: 1 };
			}
			const nextExp = expData.expData[appState.selectedPlayer.level + 1];
			return {
				levelProgressToNext: nextExp.TotalEXP - appState.selectedPlayer.exp || 0,
				levelProgressValue: nextExp.NextEXP - (nextExp.TotalEXP - appState.selectedPlayer.exp),
				levelProgressMax: nextExp.NextEXP
			};
		}
		return { levelProgressToNext: 0, levelProgressValue: 0, levelProgressMax: 1 };
	});

	const gearToAdd = $derived.by(() => {
		return Object.values(itemsData.items)
			.filter((item) => {
				// @ts-ignore
				return item.details.type_b === 'Essential_PalGear' && !item.details.disabled;
			})
			.sort((a, b) => (a.details.sort_id || Infinity) - (b.details.sort_id || Infinity));
	});

	const implantsToAdd = $derived.by(() => {
		return Object.values(itemsData.items)
			.filter((item) => {
				// @ts-ignore
				return item.id.includes('PalPassiveSkillChange');
			})
			.sort((a, b) => (a.details.sort_id || Infinity) - (b.details.sort_id || Infinity));
	});

	const miscKeysToAdd = $derived.by(() => {
		return Object.values(itemsData.items)
			.filter((item) => {
				// @ts-ignore
				return (
					!item.id.includes('PalPassiveSkillChange') &&
					item.details.type_b !== 'Essential_PalGear' &&
					item.details.type_a === 'Essential' &&
					!item.id.includes('BossDefeatReward') &&
					item.id !== 'Relic'
				);
			})
			.sort((a, b) => (a.details.sort_id || Infinity) - (b.details.sort_id || Infinity));
	});

	async function getItemIcon(staticId: string) {
		if (!staticId || staticId === 'None') return;
		const itemData = itemsData.items[staticId] || undefined;
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${itemData.details.icon}.webp`);
	}

	function clearContainer(container: ItemContainer) {
		Object.values(container.slots).forEach((slot) => {
			slot.dynamic_item = undefined;
			slot.static_id = 'None';
			slot.count = 0;
			// @ts-ignore
			slot.local_id = '00000000-0000-0000-0000-000000000000';
		});
		if (appState.selectedPlayer) {
			appState.selectedPlayer.state = EntryState.MODIFIED;
		}
	}

	function clearCommonContainer() {
		clearContainer(commonContainer);
	}

	function clearEssentialContainer() {
		clearContainer(essentialContainer);
	}

	function clearWeaponLoadOutContainer() {
		clearContainer(weaponLoadOutContainer);
	}

	function clearEquipmentArmorContainer() {
		clearContainer(playerEquipmentArmorContainer);
	}

	function clearFoodEquipContainer() {
		clearContainer(foodEquipContainer);
	}

	function clearAll() {
		clearCommonContainer();
		clearEssentialContainer();
		clearWeaponLoadOutContainer();
		clearEquipmentArmorContainer();
		clearFoodEquipContainer();
	}

	async function setCommonContainerCount() {
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		const result = await modal.showModal<number>(NumberInputModal, {
			title: 'Enter Item Count',
			value: '',
			min: 0,
			max: 9999
		});

		if (!result) return;

		Object.values(commonContainer.slots).forEach((slot) => {
			if (slot.static_id === 'None') return;
			else slot.count = result;
		});
		if (appState.selectedPlayer) {
			appState.selectedPlayer.state = EntryState.MODIFIED;
		}
	}

	async function fillCommonContainer() {
		// @ts-ignore
		const result = await modal.showModal<[string, number]>(ItemSelectModal, {
			group: 'Common',
			itemId: '',
			title: 'Select Item'
		});
		if (!result) return;
		let [static_id, count] = result;
		const itemData = itemsData.items[static_id];
		if (!itemData) return;
		count = count > itemData.details.max_stack_count ? itemData.details.max_stack_count : count;

		Object.values(commonContainer.slots).forEach((slot: ItemContainerSlot) => {
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
		if (appState.selectedPlayer) {
			appState.selectedPlayer.state = EntryState.MODIFIED;
		}
	}

	function setEssentialList(option: string) {
		if (option === 'gear') {
			fillEssentialContainer(gearToAdd);
		} else if (option === 'implants') {
			fillEssentialContainer(implantsToAdd);
		} else if (option === 'misc') {
			fillEssentialContainer(miscKeysToAdd);
		} else {
			toast.add('Invalid option selected', undefined, 'error');
		}
	}

	function fillEssentialContainer(itemList: any[]) {
		const existingKeyItems = new Set(
			Object.values(essentialContainer.slots)
				.filter((slot: ItemContainerSlot) => slot.static_id !== 'None')
				.map((slot: ItemContainerSlot) => slot.static_id)
		);

		let itemIndex = 0;
		for (const slot of Object.values(essentialContainer.slots) as ItemContainerSlot[]) {
			if (slot.static_id !== 'None') continue;

			while (itemIndex < itemList.length && existingKeyItems.has(itemList[itemIndex].id)) {
				itemIndex++;
			}
			if (itemIndex >= itemList.length) break;

			const item = itemList[itemIndex];
			slot.static_id = item.id;
			slot.count = 1;
			const itemData = itemsData.items[slot.static_id];
			if (itemData && itemData.details.dynamic) {
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
			existingKeyItems.add(slot.static_id);
			itemIndex++;
		}
		if (appState.selectedPlayer) {
			appState.selectedPlayer.state = EntryState.MODIFIED;
		}
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

	async function handleLevelIncrement(event: MouseEvent) {
		if (!appState.selectedPlayer || !appState.selectedPlayer || !appState.selectedPlayer.pals)
			return;

		let newLevel = appState.selectedPlayer.level;

		if (event.ctrlKey) {
			if (event.button === 0) {
				newLevel = Math.min(appState.selectedPlayer.level + 5, max_level);
			} else if (event.button === 1) {
				newLevel = max_level;
			} else if (event.button === 2) {
				newLevel = Math.min(appState.selectedPlayer.level + 10, max_level);
			}
		} else {
			newLevel = Math.min(appState.selectedPlayer.level + 1, max_level);
		}

		if (newLevel === appState.selectedPlayer.level) return;

		const nextLevelData = await expData.getExpDataByLevel(newLevel + 1);

		appState.selectedPlayer.level = newLevel;
		appState.selectedPlayer.exp = nextLevelData.TotalEXP - nextLevelData.NextEXP;
		appState.selectedPlayer.state = EntryState.MODIFIED;
	}

	async function handleLevelDecrement(event: MouseEvent) {
		if (!appState.selectedPlayer || !appState.selectedPlayer || !appState.selectedPlayer.pals)
			return;

		let newLevel = appState.selectedPlayer.level;

		if (event.ctrlKey) {
			if (event.button === 0) {
				newLevel = Math.max(appState.selectedPlayer.level - 5, 1);
			} else if (event.button === 1) {
				newLevel = 1;
			} else if (event.button === 2) {
				newLevel = Math.max(appState.selectedPlayer.level - 10, 1);
			}
		} else {
			newLevel = Math.max(appState.selectedPlayer.level - 1, 1);
		}

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
				<div class="flex flex-col space-y-2">
					<nav
						class="btn-group preset-outlined-surface-200-800 w-full flex-col items-center rounded-sm p-2 md:flex-row"
					>
						{#if group === 'inventory'}
							<Tooltip label="Sort Inventory">
								<button
									class="hover:bg-secondary-500/50 btn rounded-sm"
									onclick={sortCommonContainer}
								>
									<ArrowUp01 class="h-4 w-4" />
								</button>
							</Tooltip>
							<Tooltip label="Fill Inventory">
								<button
									class="hover:bg-secondary-500/50 btn rounded-sm"
									onclick={fillCommonContainer}
								>
									<PaintBucket class="h-4 w-4" />
								</button>
							</Tooltip>
							<Tooltip label="Set Inventory Count">
								<button
									class="hover:bg-secondary-500/50 btn rounded-sm"
									onclick={setCommonContainerCount}
								>
									<Hash class="h-4 w-4" />
								</button>
							</Tooltip>
							<Tooltip label="Clear Inventory">
								<button
									class="hover:bg-secondary-500/50 btn rounded-sm"
									onclick={clearCommonContainer}
								>
									<ChevronsLeftRight class="h-4 w-4" />
								</button>
							</Tooltip>
						{/if}
						{#if group === 'key_items'}
							<Tooltip label="Add All Pal Gear">
								<button
									class="hover:bg-secondary-500/50 btn rounded-sm"
									onclick={() => setEssentialList('gear')}
								>
									<PawPrint class="h-4 w-4" />
								</button>
							</Tooltip>
							<Tooltip label="Add All Implants">
								<button
									class="hover:bg-secondary-500/50 btn rounded-sm"
									onclick={() => setEssentialList('implants')}
								>
									<Activity class="h-4 w-4" />
								</button>
							</Tooltip>
							<Tooltip label="Add Other Key Items">
								<button
									class="hover:bg-secondary-500/50 btn rounded-sm"
									onclick={() => setEssentialList('misc')}
								>
									<Key class="h-4 w-4" />
								</button>
							</Tooltip>
							<Tooltip label="Clear Key Items">
								<button
									class="hover:bg-secondary-500/50 btn rounded-sm"
									onclick={clearEssentialContainer}
								>
									<ChevronsLeftRight class="h-4 w-4" />
								</button>
							</Tooltip>
						{/if}
						<Tooltip label="Clear Weapons">
							<button
								class="hover:bg-secondary-500/50 btn rounded-sm"
								onclick={clearWeaponLoadOutContainer}
							>
								<Swords class="h-4 w-4" />
							</button>
						</Tooltip>
						<Tooltip label="Clear Armor">
							<button
								class="hover:bg-secondary-500/50 btn rounded-sm"
								onclick={clearEquipmentArmorContainer}
							>
								<Shield class="h-4 w-4" />
							</button>
						</Tooltip>
						<Tooltip label="Clear Food">
							<button
								class="hover:bg-secondary-500/50 btn rounded-sm"
								onclick={clearFoodEquipContainer}
							>
								<Pizza class="h-4 w-4" />
							</button>
						</Tooltip>
						<Tooltip label="Clear All">
							<button class="hover:bg-secondary-500/50 btn rounded-sm" onclick={clearAll}>
								<Bomb class="h-4 w-4" />
							</button>
						</Tooltip>
					</nav>
					<Tabs
						listBorder="preset-outlined-surface-200-800"
						listClasses="btn-group preset-outlined-surface-200-800 w-full flex-col md:flex-row rounded-sm"
						value={group}
						onValueChange={(e) => (group = e.value)}
					>
						{#snippet list()}
							<Tabs.Control
								value="inventory"
								classes="w-full"
								base="border-none hover:bg-secondary-500/50 rounded-sm"
								labelBase="btn"
								stateActive="bg-secondary-800"
								padding="p-0"
							>
								Inventory
							</Tabs.Control>
							<Tabs.Control
								value="key_items"
								classes="w-full"
								base="border-none hover:bg-secondary-500/50 rounded-sm"
								labelBase="btn"
								stateActive="bg-secondary-800"
								padding="p-0"
							>
								Key Items
							</Tabs.Control>
						{/snippet}
						{#snippet content()}
							<Tabs.Panel value="inventory">
								<div class="max-h-[500px] overflow-y-auto 2xl:max-h-[800px]">
									<div class="m-1 grid grid-cols-6 gap-2">
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
									<div class="m-1 grid grid-cols-6 gap-2">
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
				<div class="flex h-[600px] flex-col 2xl:grid 2xl:grid-cols-[auto_1fr_auto]">
					<div class="flex flex-col space-y-2">
						<div class="flex flex-col space-y-2">
							<ItemHeader text="Weapon" />
							<div class="flex space-x-2 2xl:flex-col 2xl:space-y-2">
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
						</div>
						<div class="flex flex-col space-y-2">
							<ItemHeader text="Accessory" />
							<div class="2xl:ml-2">
								<div class="flex max-h-36 max-w-36 gap-2 2xl:grid 2xl:grid-cols-2">
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
					</div>
					<div class="hidden flex-col items-center justify-center 2xl:flex">
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
					<div class="mt-2 flex space-x-2 space-y-2 2xl:flex-col">
						<div class="flex flex-col space-y-2">
							<ItemHeader text="Head" />
							<ItemBadge
								bind:slot={headGear}
								itemGroup="Head"
								onCopyPaste={(event) => handleCopyPaste(event, headGear, false)}
								onUpdate={onItemUpdate}
							/>
						</div>
						<div class="flex flex-col space-y-2">
							<ItemHeader text="Body" />
							<ItemBadge
								bind:slot={bodyGear}
								itemGroup="Body"
								onCopyPaste={(event) => handleCopyPaste(event, bodyGear, false)}
								onUpdate={onItemUpdate}
							/>
						</div>
						<div class="flex flex-col space-y-2">
							<ItemHeader text="Shield" />
							<ItemBadge
								bind:slot={shieldGear}
								itemGroup="Shield"
								onCopyPaste={(event) => handleCopyPaste(event, shieldGear, false)}
								onUpdate={onItemUpdate}
							/>
						</div>
						<div class="flex flex-col space-y-2">
							<ItemHeader text="Glider" />
							<ItemBadge
								bind:slot={gliderGear}
								itemGroup="Glider"
								onCopyPaste={(event) => handleCopyPaste(event, gliderGear, false)}
								onUpdate={onItemUpdate}
							/>
						</div>
						<div class="flex flex-col space-y-2">
							<ItemHeader text="Sphere Module" baseClass="hidden 2xl:block" />
							<ItemHeader text="Module" baseClass="block 2xl:hidden" />
							<ItemBadge
								bind:slot={sphereModule}
								itemGroup="SphereModule"
								onCopyPaste={(event) => handleCopyPaste(event, sphereModule, false)}
								onUpdate={onItemUpdate}
							/>
						</div>
					</div>
					<div class="col-span-3 space-y-2 2xl:ml-12 2xl:mt-2">
						<ItemHeader text="Food" />
						<div class="2xl:ml-2">
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
						<div class="flex items-center">
							<Tooltip position="bottom">
								<button
									oncontextmenu={(event) => event.preventDefault()}
									class="btn hover:bg-secondary-500/25 mr-4 px-2"
									onmousedown={(event) => handleLevelDecrement(event)}
								>
									<Minus class="text-primary-500" size={16} />
								</button>
								{#snippet popup()}
									<div class="flex items-center space-x-2">
										<div class="h-6 w-6">
											<img src={staticIcons.ctrlIcon} alt="Control" class="h-full w-full" />
										</div>
										<div class="h-6 w-6">
											<img src={staticIcons.leftClickIcon} alt="Left Click" class="h-full w-full" />
										</div>
										<span class="text-xs font-bold">-5</span>
									</div>
									<div class="flex items-center space-x-2">
										<div class="h-6 w-6">
											<img src={staticIcons.ctrlIcon} alt="Control" class="h-full w-full" />
										</div>
										<div class="h-6 w-6">
											<img
												src={staticIcons.rightClickIcon}
												alt="Right Click"
												class="h-full w-full"
											/>
										</div>
										<span class="text-xs font-bold">-10</span>
									</div>
									<div class="flex items-center space-x-2">
										<div class="h-6 w-6">
											<img src={staticIcons.ctrlIcon} alt="Right Click" class="h-full w-full" />
										</div>
										<div class="h-6 w-6">
											<img
												src={staticIcons.middleClickIcon}
												alt="Middle Click"
												class="h-full w-full"
											/>
										</div>
										<span class="text-xs font-bold">Level 1</span>
									</div>
								{/snippet}
							</Tooltip>

							<div class="flex flex-col items-center justify-center">
								<span class="text-surface-400 text-sm font-bold">LEVEL</span>
								<span class="text-xl font-bold xl:text-2xl">
									<NumberFlow value={appState.selectedPlayer.level} />
								</span>
							</div>

							<Tooltip position="bottom">
								<button
									oncontextmenu={(event) => event.preventDefault()}
									class="btn hover:bg-secondary-500/25 ml-4 px-2"
									onmousedown={(event) => handleLevelIncrement(event)}
								>
									<Plus class="text-primary-500" size={16} />
								</button>
								{#snippet popup()}
									<div class="flex items-center space-x-2">
										<div class="h-6 w-6">
											<img src={staticIcons.ctrlIcon} alt="Control" class="h-full w-full" />
										</div>
										<div class="h-6 w-6">
											<img src={staticIcons.leftClickIcon} alt="Left Click" class="h-full w-full" />
										</div>
										<span class="text-xs font-bold">+5</span>
									</div>
									<div class="flex items-center space-x-2">
										<div class="h-6 w-6">
											<img src={staticIcons.ctrlIcon} alt="Control" class="h-full w-full" />
										</div>
										<div class="h-6 w-6">
											<img
												src={staticIcons.rightClickIcon}
												alt="Right Click"
												class="h-full w-full"
											/>
										</div>
										<span class="text-xs font-bold">+10</span>
									</div>
									<div class="flex items-center space-x-2">
										<div class="h-6 w-6">
											<img src={staticIcons.ctrlIcon} alt="Right Click" class="h-full w-full" />
										</div>
										<div class="h-6 w-6">
											<img
												src={staticIcons.middleClickIcon}
												alt="Middle Click"
												class="h-full w-full"
											/>
										</div>
										<span class="text-xs font-bold">Level {max_level}</span>
									</div>
								{/snippet}
							</Tooltip>
						</div>
					</div>

					<div class="grow">
						<div class="flex flex-col">
							<div class="flex space-x-2">
								<button
									class="hover:bg-secondary-500/50 hover:ring-offset-surface-900 text-start font-bold hover:ring hover:ring-offset-4"
									onclick={handleUpdateNickname}
								>
									<Edit class="h-4 w-4" />
								</button>
								<Tooltip
									label={new Date(appState.selectedPlayer.last_online_time).toLocaleString()}
								>
									<span>{appState.selectedPlayer.nickname}</span>
								</Tooltip>
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
				<Accordion
					value={sideBarExpanded}
					onValueChange={(e) => (sideBarExpanded = e.value)}
					collapsible
				>
					<Accordion.Item value="stats" controlHover="hover:bg-secondary-500/25">
						{#snippet control()}
							Stats
						{/snippet}
						{#snippet panel()}
							<PlayerStats player={appState.selectedPlayer!} />
						{/snippet}
					</Accordion.Item>
					<hr class="hr" />
					<Accordion.Item value="presets" controlHover="hover:bg-secondary-500/25">
						{#snippet control()}Presets{/snippet}
						{#snippet panel()}
							<PlayerPresets containerRef={sideBarWrapper} bind:player={appState.selectedPlayer} />
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
