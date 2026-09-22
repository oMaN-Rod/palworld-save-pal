<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { ActionGroup } from '$components/ui/actions';
	import SectionTabs, { panelId, tabId } from '$components/ui/tabs/SectionTabs.svelte';
	import { layout } from '$utils/layout.svelte';
	import { getAppState, getToastState, getModalState } from '$states';
	import {
		EntryState,
		EPalPlayerEquipItemSlotType,
		PLAYER_EQUIP_ACCESSORY_SLOTS,
		SPHERE_MODULE_SLOT,
		type ItemContainerSlot,
		type ItemContainer
	} from '$types';
	import { MAX_LEVEL } from '$lib/constants';
	import { itemsData, expData } from '$lib/data';
	import { TextInputModal, NumberInputModal, ItemSelectModal } from '$components/modals';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import PlayerGear from './components/PlayerGear.svelte';
	import PlayerStatsPanel from './components/PlayerStatsPanel.svelte';
	import PlayerInventory from './components/PlayerInventory.svelte';
	import { buildPlayerActions } from './playerActions';

	const appState = getAppState();
	const toast = getToastState();
	const modal = getModalState();

	const maxLevel = $derived(appState.settings.cheat_mode ? 99 : MAX_LEVEL);

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
	let group: 'inventory' | 'key_items' = $state('inventory');

	let health = $state(500);

	let foodSlotCount = $derived.by(() => {
		let slotCount = 0;
		Object.values(essentialContainer.slots).forEach((slot) => {
			if (slot.static_id.includes('AutoMealPouch_Tier')) {
				const foodCount = parseInt(slot.static_id.slice(-1));
				slotCount = foodCount > slotCount ? foodCount : slotCount;
			}
		});
		return Math.max(slotCount, 3);
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
			title: m.enter_item_count(),
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
			title: m.select_entity({ entity: c.item })
		});
		if (!result) return;
		let [static_id, count] = result;
		const itemData = itemsData.getByKey(static_id);
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
			toast.add(m.invalid_option_selected(), undefined, 'error');
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
			const itemData = itemsData.getByKey(slot.static_id);
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
			const itemData = itemsData.getByKey(slot.static_id);
			if (itemData) {
				itemName = itemData.info.localized_name;
			}
			toast.add(m.item_copied({ name: itemName }));
		} else {
			appState.clipboardItem = null;
			toast.add(m.clipboard_cleared());
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
			toast.add(m.cannot_paste_here(), undefined, 'warning');
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
			for (let i = 0; i < container.slot_num; i++) {
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
			for (let i = 0; i < EPalPlayerEquipItemSlotType.Max; i++) {
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

			headGear = containerSlots[EPalPlayerEquipItemSlotType.Head];
			bodyGear = containerSlots[EPalPlayerEquipItemSlotType.Body];
			shieldGear = containerSlots[EPalPlayerEquipItemSlotType.Shield];
			gliderGear = containerSlots[EPalPlayerEquipItemSlotType.Glider];
			sphereModule = containerSlots[SPHERE_MODULE_SLOT];
			accessoryGear = PLAYER_EQUIP_ACCESSORY_SLOTS.map((slot) => containerSlots[slot]);
			playerEquipmentArmorContainer.slots = containerSlots;
		}
	}

	async function sortCommonContainer() {
		if (appState.selectedPlayer) {
			const sortedSlots = commonContainer.slots.map((slot) => {
				if (slot.static_id !== 'None') {
					const itemData = itemsData.getByKey(slot.static_id);
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
				newLevel = Math.min(appState.selectedPlayer.level + 5, maxLevel);
			} else if (event.button === 1) {
				newLevel = maxLevel;
			} else if (event.button === 2) {
				newLevel = Math.min(appState.selectedPlayer.level + 10, maxLevel);
			}
		} else {
			newLevel = Math.min(appState.selectedPlayer.level + 1, maxLevel);
		}

		if (newLevel === appState.selectedPlayer.level) return;

		try {
			const nextLevelData = await expData.getExpDataByLevel(newLevel + 1);

			appState.selectedPlayer.level = newLevel;
			appState.selectedPlayer.exp = nextLevelData.TotalEXP - nextLevelData.NextEXP;
			appState.selectedPlayer.state = EntryState.MODIFIED;
		} catch (error) {
			console.error('Error incrementing player level:', error);
		}
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

		try {
			const newLevelData = await expData.getExpDataByLevel(newLevel + 1);

			appState.selectedPlayer.level = newLevel;
			appState.selectedPlayer.exp = newLevelData.TotalEXP - newLevelData.NextEXP;
			appState.selectedPlayer.state = EntryState.MODIFIED;
		} catch (error) {
			console.error('Error decrementing player level:', error);
		}
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
			title: m.change_player_name(),
			value: appState.selectedPlayer.nickname
		});
		if (result) {
			appState.selectedPlayer.nickname = result;
			appState.selectedPlayer.state = EntryState.MODIFIED;
		}
	}

	const SECTION_ID_PREFIX = 'player';

	// Phone reading order, not the desktop grid's.
	const sections = $derived([
		{ id: 'stats', label: m.stats(), body: statsSection },
		{ id: 'inventory', label: m.inventory(), body: inventorySection },
		{ id: 'gear', label: m.gear(), body: gearSection }
	]);

	let activeSection = $state('stats');

	const playerActions = $derived(
		buildPlayerActions({
			group,
			sortCommonContainer,
			fillCommonContainer,
			setCommonContainerCount,
			clearCommonContainer,
			setEssentialList,
			clearEssentialContainer,
			clearWeaponLoadOutContainer,
			clearEquipmentArmorContainer,
			clearFoodEquipContainer,
			clearAll
		})
	);
</script>

{#snippet inventorySection()}
	<PlayerInventory
		{commonContainer}
		{essentialContainer}
		bind:group
		onUpdate={onItemUpdate}
		onCopyPaste={handleCopyPaste}
	/>
{/snippet}

{#snippet gearSection()}
	<PlayerGear
		{weaponLoadOutContainer}
		{foodEquipContainer}
		{accessoryGear}
		{headGear}
		{bodyGear}
		{shieldGear}
		{gliderGear}
		{sphereModule}
		onUpdate={onItemUpdate}
		onCopyPaste={(event, slot) => handleCopyPaste(event, slot, false)}
	/>
{/snippet}

{#snippet statsSection()}
	<PlayerStatsPanel
		player={appState.selectedPlayer!}
		{maxLevel}
		bind:health
		onLevelIncrement={handleLevelIncrement}
		onLevelDecrement={handleLevelDecrement}
		onUpdateNickname={handleUpdateNickname}
	/>
{/snippet}

{#if appState.selectedPlayer}
	<div class="flex h-full flex-col overflow-auto">
		<div class="ml-2 flex">
			<ActionGroup
				id="quick-actions"
				actions={playerActions}
				title={m.quick_actions()}
				class="mr-2"
			/>
			{#if layout.phone}
				<div class="flex w-full min-w-0 flex-col">
					<SectionTabs
						tabs={sections.map(({ id, label }) => ({ id, label }))}
						bind:active={activeSection}
						label={m.player_sections()}
						idPrefix={SECTION_ID_PREFIX}
					/>
					{#each sections as section (section.id)}
						{#if section.id === activeSection}
							<div
								id={panelId(SECTION_ID_PREFIX, section.id)}
								role="tabpanel"
								aria-labelledby={tabId(SECTION_ID_PREFIX, section.id)}
								tabindex="0"
								data-testid="player-{section.id}"
								class="min-w-0 flex-1 overflow-y-auto p-2"
							>
								{@render section.body()}
							</div>
						{/if}
					{/each}
				</div>
			{:else}
				<div
					class="@container grid w-full grid-cols-[auto_1fr] gap-4 pr-4 xl:grid-cols-[auto_1fr_24rem]"
				>
					<div data-testid="player-inventory" class="flex flex-col space-y-2">
						{@render inventorySection()}
					</div>
					<div
						data-testid="player-gear"
						class="flex min-h-0 flex-col 2xl:grid 2xl:grid-cols-[auto_1fr_auto]"
					>
						{@render gearSection()}
					</div>
					<div data-testid="player-stats">
						{@render statsSection()}
					</div>
				</div>
			{/if}
		</div>
	</div>
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2 flex items-center gap-2">
			<Icon icon="tabler:rocket" class="text-secondary-400 h-6 w-6" />
			{m.select_entity_to_edit({ entity: c.player })}
		</h2>
	</div>
{/if}
