<script lang="ts">
	import { Card, ItemHeader, Progress } from '$components/ui';
	import { getAppState, getToastState, getSocketState, getModalState } from '$states';
	import {
		EntryState,
		type ItemContainerSlot,
		type ItemContainer,
		type Pal,
		MessageType
	} from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { itemsData, expData, palsData } from '$lib/data';
	import { Tabs, Accordion } from '@skeletonlabs/skeleton-svelte';
	import { Tooltip } from '$components/ui';
	import {
		ItemBadge,
		PlayerPresets,
		PalBadge,
		PalSelectModal,
		PlayerStats,
		PlayerHealthBadge
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
		Plus
	} from 'lucide-svelte';
	import { assetLoader } from '$utils';

	const ws = getSocketState();
	const appState = getAppState();
	const toast = getToastState();
	const modal = getModalState();

	let otomoContainer: Record<string, Pal> = $state({});
	let commonContainer: ItemContainer = $state({ id: '', type: '', slots: [] });
	let essentialContainer: ItemContainer = $state({ id: '', type: '', slots: [] });
	let weaponLoadOutContainer: ItemContainer = $state({ id: '', type: '', slots: [] });
	let playerEquipmentArmorContainer: ItemContainer = $state({ id: '', type: '', slots: [] });
	let foodEquipContainer: ItemContainer = $state({ id: '', type: '', slots: [] });
	let headGear: ItemContainerSlot = $state({
		id: '',
		static_id: '',
		slot_index: 0,
		type: '',
		count: 0
	});
	let bodyGear: ItemContainerSlot = $state({
		id: '',
		static_id: '',
		slot_index: 0,
		type: '',
		count: 0
	});
	let shieldGear: ItemContainerSlot = $state({
		id: '',
		static_id: '',
		slot_index: 0,
		type: '',
		count: 0
	});
	let gliderGear: ItemContainerSlot = $state({
		id: '',
		static_id: '',
		slot_index: 0,
		type: '',
		count: 0
	});
	let accessoryGear: ItemContainerSlot[] = $state([]);
	let group = $state('inventory');
	let foodSlotCount: number = 0;
	let sideBarExpanded: string[] = $state(['stats']);
	let sideBarWrapper: HTMLDivElement | null = $state(null);

	let health = $state(500);

	let { levelProgressToNext, levelProgressValue, levelProgressMax } = $derived.by(() => {
		if (appState.selectedPlayer) {
			if (appState.selectedPlayer.level === 55) {
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
		if (!staticId) return;
		const itemData = await itemsData.searchItems(staticId);
		if (!itemData) {
			console.error(`Item data not found for static id: ${staticId}`);
			return;
		}
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/${itemData.details.icon}.png`);
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
			const itemData = await itemsData.searchItems(slot.static_id);
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
			for (let i = 0; i < 42; i++) {
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
			for (let i = 0; i < 100; i++) {
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
					if (slot.static_id.includes('AutoMealPouch_Tier')) {
						const foodCount = parseInt(slot.static_id.slice(-1));
						foodSlotCount = foodCount > foodSlotCount ? foodCount : foodSlotCount;
					}
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
			for (let i = 0; i < 8; i++) {
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
			accessoryGear = containerSlots.slice(2, 4).concat(containerSlots.slice(6, 8));
			playerEquipmentArmorContainer.slots = containerSlots;
		}
	}

	function loadOtomoContainer() {
		if (appState.selectedPlayer && appState.selectedPlayer.pals) {
			const container_id = appState.selectedPlayer.otomo_container_id;

			const otomoEntries = Object.entries(appState.selectedPlayer.pals).filter(
				([_, pal]) => pal.storage_id === container_id
			);

			const allSlots = Array(5)
				.fill(null)
				.map((_, index) => {
					const existingPal = otomoEntries.find(([_, pal]) => pal.storage_slot === index);
					if (existingPal) {
						return existingPal;
					} else {
						const emptyPalId = `empty-${index}`;
						return [emptyPalId, { character_id: 'None' }];
					}
				});

			// Convert the array back to an object
			otomoContainer = Object.fromEntries(allSlots);
		}
	}

	async function sortCommonContainer() {
		if (appState.selectedPlayer) {
			const sortedSlots = await Promise.all(
				commonContainer.slots.map(async (slot) => {
					if (slot.static_id !== 'None') {
						const itemData = await itemsData.searchItems(slot.static_id);
						return { ...slot, sort_id: itemData?.details.sort_id ?? Infinity };
					}
					return { ...slot, sort_id: Infinity };
				})
			);

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

		const newLevel = Math.min(appState.selectedPlayer.level + 1, 55);
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

	function handleMoveToPalbox(pal: Pal) {
		if (appState.selectedPlayer) {
			const message = {
				type: MessageType.MOVE_PAL,
				data: {
					player_id: appState.selectedPlayer.uid,
					pal_id: pal.instance_id,
					container_id: appState.selectedPlayer.pal_box_id
				}
			};
			ws.send(JSON.stringify(message));
		}
	}

	async function handleDeletePal(pal: Pal) {
		const palData = await palsData.getPalInfo(pal.character_id);
		const confirmed = await modal.showConfirmModal({
			title: 'Delete Pal(s)',
			message: `Are you sure you want to delete ${pal.nickname || palData?.localized_name}?`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});

		if (appState.selectedPlayer && appState.selectedPlayer.pals && confirmed) {
			const data = {
				player_id: appState.selectedPlayer.uid,
				pal_ids: [pal.instance_id]
			};

			const message = {
				type: MessageType.DELETE_PALS,
				data
			};
			ws.send(JSON.stringify(message));
			appState.selectedPlayer.pals = Object.fromEntries(
				Object.entries(appState.selectedPlayer.pals).filter(([id]) => pal.instance_id !== id)
			);
		}
	}

	async function handleAddPal() {
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		const [selectedPal, nickname] = await modal.showModal<string>(PalSelectModal, {
			title: 'Add a new Pal'
		});
		const palData = await palsData.getPalInfo(selectedPal);
		const message = {
			type: MessageType.ADD_PAL,
			data: {
				player_id: appState.selectedPlayer.uid,
				pal_code_name: selectedPal,
				nickname: nickname || `[New] ${palData?.localized_name}`,
				container_id: appState.selectedPlayer.otomo_container_id
			}
		};
		ws.send(JSON.stringify(message));
	}

	$effect(() => {
		if (appState.selectedPlayer) {
			loadCommonContainer();
			loadEssentialContainer();
			loadFoodContainer();
			loadWeaponLoadoutContainer();
			loadPlayerEquipmentArmorContainer();
			loadOtomoContainer();
			health = 500 + appState.selectedPlayer.status_point_list.max_hp * 100;
		}
	});
</script>

{#if appState.selectedPlayer}
	<div class="flex h-full flex-col overflow-auto">
		<div class="ml-2 flex">
			<!-- Main content wrapper -->
			<div class="grid w-full grid-cols-[auto_1fr] gap-4 pr-[340px]">
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
								<div class="grid grid-cols-6 gap-2">
									{#each Object.values(commonContainer.slots) as _, index}
										<ItemBadge
											bind:slot={commonContainer.slots[index]}
											itemGroup="Common"
											onCopyPaste={(event) => handleCopyPaste(event, commonContainer.slots[index])}
											onUpdate={onItemUpdate}
										/>
									{/each}
								</div>
							</Tabs.Panel>
							<Tabs.Panel value="key_items">
								<div class="max-h-[500px] overflow-auto">
									<div class="grid grid-cols-6 gap-2">
										{#each Object.values(essentialContainer.slots) as _, index}
											<ItemBadge
												bind:slot={essentialContainer.slots[index]}
												itemGroup="KeyItem"
												onUpdate={onItemUpdate}
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
								<img src={icon} alt={headGear.static_id} class="h-12 w-12 xl:h-16 xl:w-16" />
							{/await}
						</span>
						<span class="h-2/3">
							{#await getItemIcon(bodyGear.static_id) then icon}
								<img src={icon} alt={bodyGear.static_id} class="h-56 w-56 xl:h-64 xl:w-64" />
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
			<div class="fixed right-2 top-[60px] w-80 flex-none" bind:this={sideBarWrapper}>
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
		<!-- Party -->
		<div class="flex">
			<Card rounded="rounded-none" class="m-2 mt-4 px-4 py-2.5">
				<div class="flex">
					<h6 class="h6 mr-4">Party</h6>
					<div class="flex flex-col">
						<div class="flex flex-row space-x-4 xl:space-x-8">
							{#each Object.values(otomoContainer) as pal}
								<PalBadge
									bind:pal={otomoContainer[pal.instance_id]}
									onMoveToPalbox={() => handleMoveToPalbox(pal)}
									onDelete={() => handleDeletePal(pal)}
									onAdd={() => handleAddPal()}
								/>
							{/each}
						</div>
					</div>
				</div>
			</Card>
		</div>
	</div>
{/if}
