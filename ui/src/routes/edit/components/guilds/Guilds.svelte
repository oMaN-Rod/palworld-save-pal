<script lang="ts">
	import { palsData, buildingsData, itemsData, presetsData } from '$lib/data';
	import { getAppState, getModalState, getToastState } from '$states';
	import { Input, List, Tooltip, TooltipButton } from '$components/ui';
	import {
		type ItemContainer,
		type Pal,
		type ItemContainerSlot,
		MessageType,
		EntryState,
		BuildingTypeA,
		Rarity
	} from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { Ambulance, X, ReplaceAll, Plus, Trash, Bandage, Play, RefreshCcw } from 'lucide-svelte';
	import { DebugButton, ItemBadge, PalBadge, StoragePresets } from '$components';
	import { PalSelectModal, NumberInputModal, PalPresetSelectModal } from '$components/modals';
	import { assetLoader, debounce, deepCopy, formatNickname } from '$utils';
	import { cn } from '$theme';
	import { TextInputModal } from '$components/modals';
	import { staticIcons } from '$types/icons';
	import { send, sendAndWait } from '$lib/utils/websocketUtils';
	interface PalWithBaseId {
		pal: Pal;
		baseId: string;
	}

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();

	const VISIBLE_PAGE_BUBBLES = 16;

	let selectedPals: string[] = $state([]);
	let palSearchQuery = $state('');
	let currentPage = $state(1);
	let filteredPals: PalWithBaseId[] = $state([]);
	let activeTab: 'pals' | 'storage' | 'guildChest' = $state('pals');
	let currentStorageContainer: (ItemContainer & { slots: ItemContainerSlot[] }) | undefined =
		$state(undefined);
	let selectedInventoryItem: string = $state('');
	let inventorySearchQuery: string = $state('');

	const playerGuild = $derived.by(() => {
		if (appState.selectedPlayer?.guild_id) {
			return appState.guilds[appState.selectedPlayer.guild_id];
		}
	});

	const guildChestIcon = $derived.by(() => {
		if (!playerGuild?.guild_chest) return null;
		const building = buildingsData.buildings['GuildChest'];
		if (building) {
			return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${building.icon}.png`);
		}
		return staticIcons.unknownIcon;
	});

	const guildBases = $derived.by(() => {
		if (playerGuild) {
			return playerGuild.bases;
		}
	});

	const totalPages = $derived(Object.keys(guildBases || {}).length);

	const visiblePageStart = $derived(
		Math.max(
			1,
			Math.min(
				currentPage - Math.floor(VISIBLE_PAGE_BUBBLES / 2),
				totalPages - VISIBLE_PAGE_BUBBLES + 1
			)
		)
	);

	const visiblePageEnd = $derived(
		Math.min(visiblePageStart + VISIBLE_PAGE_BUBBLES - 1, totalPages)
	);

	const visiblePages = $derived(
		Array.from({ length: visiblePageEnd - visiblePageStart + 1 }, (_, i) => visiblePageStart + i)
	);

	const currentBase = $derived.by(() => {
		if (!guildBases) return null;
		const baseEntries = Object.entries(guildBases);
		return baseEntries[currentPage - 1] || null;
	});

	const ignoreKeys = ['None', 'Empty', 'TreasureBox', 'PalEgg', 'CommonDropItem'];

	const currentBaseStorageContainers = $derived.by(() => {
		if (!currentBase) return null;
		const [_, base] = currentBase;
		return Object.values(base.storage_containers)
			.filter(
				(container) =>
					(container.slot_num !== 0 &&
						!ignoreKeys.some((key) => container.key.includes(key)) &&
						(container.slots.length === 0 ||
							container.slots.some((s) => s.static_id !== 'None'))) ||
					(container.slots.some((s) => {
						const itemData = itemsData.items[s.static_id];
						return (
							s.static_id.toLowerCase().includes(selectedInventoryItem.toLowerCase()) ||
							(itemData &&
								itemData.info.localized_name
									.toLowerCase()
									.includes(selectedInventoryItem.toLowerCase()))
						);
					}) &&
						container.slots.some((s) => {
							const itemData = itemsData.items[s.static_id];
							return (
								s.static_id.toLowerCase().includes(inventorySearchQuery.toLowerCase()) ||
								(itemData &&
									itemData.info.localized_name
										.toLowerCase()
										.includes(inventorySearchQuery.toLowerCase()))
							);
						}))
			)
			.sort((a, b) => a.key.localeCompare(b.key));
	});

	type InventoryInfo = {
		containers: Record<string, number>;
		total_count: number;
	};

	const currentBaseInventory = $derived.by(() => {
		if (!currentBase) return { current: [] };
		const [_, base] = currentBase;
		let inventoryItems: Record<string, InventoryInfo> = {};
		for (const container of Object.values(base.storage_containers)) {
			for (const slot of container.slots) {
				if (slot.static_id !== 'None') {
					if (!inventoryItems[slot.static_id]) {
						inventoryItems[slot.static_id] = {
							containers: {},
							total_count: 0
						};
					}
					inventoryItems[slot.static_id].containers[container.key] =
						(inventoryItems[slot.static_id].containers[container.key] || 0) + slot.count;
					inventoryItems[slot.static_id].total_count += slot.count;
				}
			}
		}
		const items = Object.entries(inventoryItems)
			.filter(([static_id, _]) => {
				const itemData = itemsData.items[static_id];
				return (
					static_id.toLowerCase().includes(inventorySearchQuery.toLowerCase()) ||
					(itemData &&
						itemData.info.localized_name.toLowerCase().includes(inventorySearchQuery.toLowerCase()))
				);
			})
			.map(([static_id, info]) => ({
				static_id,
				containers: info.containers,
				total_count: info.total_count
			}))
			.sort((a, b) => {
				const itemA = itemsData.items[a.static_id];
				const itemB = itemsData.items[b.static_id];
				if (itemA && itemB) {
					return itemA.info.localized_name.localeCompare(itemB.info.localized_name);
				}
				return a.static_id.localeCompare(b.static_id);
			});
		return {
			current: items
		};
	});

	const currentStorageContainerIcon = $derived.by(() => {
		if (!currentStorageContainer) return null;
		const building = buildingsData.buildings[currentStorageContainer.key];
		if (building) {
			return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${building.icon}.png`);
		}
		return staticIcons.unknownIcon;
	});

	const currentPageItems = $derived.by(() => {
		if (!currentBase) return [];
		const [baseId, base] = currentBase;

		if (palSearchQuery) {
			return filteredPals;
		}

		return Array(base.slot_count)
			.fill(undefined)
			.map((_, index) => {
				const existingPal = Object.values(base.pals).find((p) => p.storage_slot === index);
				if (existingPal) {
					return {
						pal: existingPal,
						baseId: baseId
					};
				}
				return {
					pal: {
						character_id: 'None',
						character_key: 'None',
						storage_slot: index,
						instance_id: `empty-${index}`,
						storage_id: base.container_id
					},
					baseId: baseId
				} as PalWithBaseId;
			});
	});

	const debouncedFilterPals = debounce(filterPals, 300);

	function fixStupidTypos(key: string) {
		switch (key) {
			case 'Stonepit':
				return 'StonePit';
			case 'bone':
				return 'Bone';
			default:
				return key;
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.target instanceof HTMLInputElement) return;

		if (event.key === 'ArrowLeft' || event.key === 'q' || event.key === 'Q') {
			decrementPage();
		} else if (event.key === 'ArrowRight' || event.key === 'e' || event.key === 'E') {
			incrementPage();
		}
	}

	function decrementPage() {
		if (currentPage > 1) {
			currentPage--;
		} else {
			currentPage = totalPages;
		}
		currentStorageContainer = undefined;
		inventorySearchQuery = '';
		selectedInventoryItem = '';
	}

	function incrementPage() {
		if (currentPage < totalPages) {
			currentPage++;
		} else {
			currentPage = 1;
		}
		currentStorageContainer = undefined;
		inventorySearchQuery = '';
		selectedInventoryItem = '';
	}

	function handlePalSelect(pal: Pal, event: MouseEvent) {
		if (!pal || pal.character_id === 'None') return;
		if (event.ctrlKey || event.metaKey) {
			if (selectedPals.includes(pal.instance_id)) {
				selectedPals = selectedPals.filter((id) => id !== pal.instance_id);
			} else {
				selectedPals = [...selectedPals, pal.instance_id];
			}
		}
	}

	async function handleAddPal(baseId: string, index?: number) {
		if (!appState.selectedPlayer || !guildBases) return;
		const base = guildBases[baseId];
		if (!base) return;

		// @ts-ignore
		const result = await modal.showModal<[string, string] | undefined>(PalSelectModal, {
			title: `Add a new Pal to Base ${currentPage}`
		});
		if (!result) return;

		const [selectedPal, nickname] = result;
		const palData = palsData.pals[selectedPal];

		send(MessageType.ADD_PAL, {
			guild_id: playerGuild?.id,
			base_id: baseId,
			character_id: selectedPal,
			nickname:
				nickname || formatNickname(palData?.localized_name, appState.settings.new_pal_prefix),
			container_id: base.container_id,
			storage_slot: index
		});
	}

	async function handleClonePal(item: PalWithBaseId) {
		if (!guildBases) return;
		const base = guildBases[item.baseId];
		if (!base) return;

		const maxClones = base.slot_count - Object.keys(base.pals).length;
		if (maxClones === 0) {
			toast.add('There are no slots available in this base.', 'Error', 'error');
			return;
		}

		// @ts-ignore
		const result = await modal.showModal<number>(NumberInputModal, {
			title: 'How many clones?',
			message: `There are ${maxClones} slots available in this base.`,
			value: 1,
			min: 0,
			max: maxClones
		});
		if (!result) return;

		for (let i = 0; i < result; i++) {
			const clonedPal = deepCopy(item.pal);
			clonedPal.nickname = formatNickname(
				clonedPal.nickname || clonedPal.name || clonedPal.character_id,
				appState.settings.clone_prefix
			);

			send(MessageType.CLONE_PAL, {
				guild_id: playerGuild!.id,
				base_id: item.baseId,
				pal: clonedPal
			});
		}
	}

	async function deleteSelectedPals() {
		if (selectedPals.length === 0) return;

		const confirmed = await modal.showConfirmModal({
			title: `Delete Pal${selectedPals.length > 1 ? 's' : ''}`,
			message: `Are you sure you want to delete the ${selectedPals.length} selected pal${selectedPals.length == 1 ? '' : 's'}?`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});

		if (confirmed) {
			// get base id based on current page
			const baseId = currentBase ? currentBase[0] : '';
			send(MessageType.DELETE_PALS, {
				guild_id: playerGuild?.id,
				base_id: baseId,
				pal_ids: selectedPals
			});

			playerGuild!.bases[baseId].pals = Object.fromEntries(
				Object.entries(playerGuild!.bases[baseId].pals).filter(([id]) => !selectedPals.includes(id))
			);
		}

		selectedPals = [];
	}

	async function handleDeletePal(baseId: string, pal: Pal) {
		const confirmed = await modal.showConfirmModal({
			title: 'Delete Pal',
			message: `Are you sure you want to delete ${pal.nickname || pal.name}?`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});

		if (appState.selectedPlayer && confirmed) {
			send(MessageType.DELETE_PALS, {
				guild_id: playerGuild?.id,
				base_id: baseId,
				pal_ids: [pal.instance_id]
			});
		}
		playerGuild!.bases[baseId].pals = Object.fromEntries(
			Object.entries(playerGuild!.bases[baseId].pals).filter(
				([_, p]) => p.instance_id !== pal.instance_id
			)
		);
	}

	function filterPals() {
		if (!guildBases || !palSearchQuery) return;

		filteredPals = Object.entries(guildBases).flatMap(([baseId, base]) =>
			Object.values(base.pals)
				.filter((pal) => {
					return (
						pal.name.toLowerCase().includes(palSearchQuery.toLowerCase()) ||
						pal.nickname?.toLowerCase().includes(palSearchQuery.toLowerCase()) ||
						pal.character_id.toLowerCase().includes(palSearchQuery.toLowerCase())
					);
				})
				.map((pal) => ({
					pal: pal,
					baseId: baseId
				}))
		);
	}

	function handleSelectAll() {
		if (!currentBase) return;
		const [_, base] = currentBase;

		const basePalIds = Object.values(base.pals).map((pal) => pal.instance_id);

		if (selectedPals.length === basePalIds.length) {
			selectedPals = [];
		} else {
			selectedPals = [...basePalIds];
		}
	}

	async function healSelectedPals() {
		if (!guildBases || selectedPals.length === 0) return;
		send(MessageType.HEAL_PALS, [...selectedPals]);

		Object.values(guildBases).forEach((base) => {
			Object.values(base.pals).forEach((pal) => {
				if (selectedPals.includes(pal.instance_id)) {
					pal.hp = pal.max_hp;
					pal.sanity = 100;
					const palData = palsData.pals[pal.character_key];
					if (palData) {
						pal.stomach = palData.max_full_stomach;
					}
				}
			});
		});

		selectedPals = [];
	}

	$effect(() => {
		if (palSearchQuery) {
			debouncedFilterPals();
		}
	});

	$effect(() => {
		window.addEventListener('keydown', handleKeydown);
		return () => {
			window.removeEventListener('keydown', handleKeydown);
		};
	});

	$effect(() => {
		if (currentPage > totalPages && totalPages > 0) {
			currentPage = totalPages;
		}
	});

	$effect(() => {
		if (inventorySearchQuery !== '') {
			selectedInventoryItem = '';
		}
	});

	function handleHealAll() {
		if (!guildBases || !playerGuild || !currentBase) return;
		send(MessageType.HEAL_ALL_PALS, {
			guild_id: playerGuild.id,
			base_id: currentBase[0]
		});
		Object.values(guildBases).forEach((base) => {
			Object.values(base.pals).forEach((pal) => {
				pal.hp = pal.max_hp;
				pal.sanity = 100;
				pal.is_sick = false;
				const palData = palsData.pals[pal.character_key];
				if (palData) {
					pal.stomach = palData.max_full_stomach;
				}
			});
		});
	}

	function handleSelectStorageContainer(container: ItemContainer): void {
		let containerSlots = [];
		for (let i = 0; i < container.slot_num; i++) {
			const slot = container.slots.find((slot: ItemContainerSlot) => slot.slot_index === i);
			if (!slot) {
				const emptySlot = {
					static_id: 'None',
					slot_index: i,
					count: 0,
					dynamic_item: undefined
				};
				containerSlots.push(emptySlot);
			} else {
				containerSlots.push(slot);
			}
		}
		container.slots = containerSlots;
		currentStorageContainer = container;
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
	}

	function pasteItem(slot: ItemContainerSlot) {
		if (appState.clipboardItem) {
			slot.static_id = appState.clipboardItem.static_id;
			slot.count = appState.clipboardItem.count;
			slot.dynamic_item = appState.clipboardItem.dynamic_item;
			if (slot.dynamic_item) {
				slot.dynamic_item.local_id = '00000000-0000-0000-0000-000000000000';
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

	async function handleSelectPreset() {
		const selectedPalsData = selectedPals.map((id) => {
			const pal = Object.values(currentBase![1].pals).find((p) => p.instance_id === id);
			return {
				character_id: pal?.character_id,
				character_key: pal?.character_key
			};
		});
		// @ts-ignore
		const result = await modal.showModal<string>(PalPresetSelectModal, {
			title: 'Select preset',
			selectedPals: selectedPalsData
		});
		if (!result) return;

		const presetProfile = presetsData.presetProfiles[result];

		selectedPals.forEach((id) => {
			const pal = Object.values(currentBase![1].pals).find((p) => p.instance_id === id);
			if (pal) {
				for (const [key, value] of Object.entries(presetProfile.pal_preset!)) {
					if (key === 'character_id') continue;
					if (key === 'lock' && value) {
						pal.character_id = presetProfile.pal_preset?.character_id as string;
					} else if (value) {
						(pal as Record<string, any>)[key] = value;
					}
				}
				pal.state = EntryState.MODIFIED;
			}
		});
	}

	function handleSelectGuildChest() {
		if (playerGuild?.guild_chest) {
			let chestSlots = [];
			for (let i = 0; i < playerGuild.guild_chest.slot_num; i++) {
				const slot = playerGuild.guild_chest.slots.find((slot) => slot.slot_index === i);
				if (!slot) {
					const emptySlot = {
						static_id: 'None',
						slot_index: i,
						count: 0,
						dynamic_item: undefined
					};
					chestSlots.push(emptySlot);
				} else {
					chestSlots.push(slot);
				}
			}
			playerGuild.guild_chest.slots = chestSlots;
		}
		activeTab = 'guildChest';
	}

	function getItemBackground(rarity: Rarity): string {
		switch (rarity) {
			case Rarity.Uncommon:
				return 'bg-linear-to-tl from-green-500/50';
			case Rarity.Rare:
				return 'bg-linear-to-tl from-blue-500/50';
			case Rarity.Epic:
				return 'bg-linear-to-tl from-purple-500/50';
			case Rarity.Legendary:
				return 'bg-linear-to-tl from-yellow-500/50';
			default:
				return '';
		}
	}

	async function handleEditGuildName() {
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: 'Edit Guild Name',
			value: playerGuild!.name
		});
		if (!result) return;
		playerGuild!.name = result;
		playerGuild!.state = EntryState.MODIFIED;
	}
</script>

{#if appState.selectedPlayer}
	{#if !playerGuild}
		<div class="flex w-full items-center justify-center">
			<h2 class="h2">No Guild found</h2>
		</div>
	{:else if !guildBases || Object.values(guildBases).length === 0}
		<div class="flex w-full items-center justify-center space-x-4">
			<h2 class="h2">No Guild Bases found</h2>
			<img src={staticIcons.sadIcon} alt="Sad" class="h-18 w-18" />
		</div>
	{:else}
		<div class="grid h-full w-full grid-cols-[25%_1fr]">
			<!-- Left Controls -->
			<div class="shrink-0 space-y-2 p-4">
				<div class="flex">
					<button class="btn px-0 text-start" onclick={handleEditGuildName}>
						<h4 class="h4 hover:text-secondary-500">{playerGuild!.name}</h4>
					</button>
					{#if playerGuild && appState.settings.debug_mode}
						<DebugButton href={`/debug?guildId=${playerGuild.id}`} />
					{/if}
				</div>

				<div class="flex">
					<h5 class="h5 font-light">Base {currentPage}</h5>
					{#if playerGuild && currentBase && appState.settings.debug_mode}
						<DebugButton
							iconClass="h-4 w-4"
							href={`/debug?guildId=${playerGuild.id}&baseId=${currentBase[1].id}`}
						/>
					{/if}
					<Tooltip label="Delete entire guild">
						<button 
							class="btn hover:bg-red-500/50 p-2"
							onclick={async () => {
								const confirmed = await modal.showConfirmModal({
									title: 'Delete Guild',
									message: 'Are you sure you want to delete this guild? This action cannot be undone.',
									confirmText: 'Delete',
									cancelText: 'Cancel',
								});
								if (confirmed) {
									const message = {
										type: MessageType.DELETE_GUILD,
										data: {
											guild_id: playerGuild?.id
										}
									};
									const response = await ws.sendAndWait(message);
									if (response.success) {
										toast.add('Guild deleted successfully', 'Success');
									} else {
										toast.add('Failed to delete guild', 'Error');
									}
								}
							}}
						>
							<Trash class="h-4 w-4" />
						</button>
					</Tooltip>
				</div>

				<nav
					class="btn-group preset-outlined-surface-200-800 w-full flex-col rounded-sm p-2 md:flex-row"
				>
					<button
						class={cn(
							'btn hover:bg-secondary-500/50 w-1/3 rounded-sm',
							activeTab == 'pals' ? 'bg-secondary-800' : ''
						)}
						onclick={() => {
							activeTab = 'pals';
							inventorySearchQuery = '';
							selectedInventoryItem = '';
						}}
					>
						<span>Pals</span>
					</button>
					<button
						class={cn(
							'btn hover:bg-secondary-500/50 w-1/3 rounded-sm',
							activeTab == 'storage' ? 'bg-secondary-800' : ''
						)}
						onclick={() => {
							activeTab = 'storage';
							inventorySearchQuery = '';
							selectedInventoryItem = '';
						}}
					>
						<span>Storage</span>
					</button>
					<button
						class={cn(
							'btn hover:bg-secondary-500/50 w-1/3 rounded-sm',
							activeTab == 'guildChest' ? 'bg-secondary-800' : ''
						)}
						onclick={() => {
							inventorySearchQuery = '';
							selectedInventoryItem = '';
							handleSelectGuildChest();
						}}
					>
						<span>Guild Chest</span>
					</button>
				</nav>
				{#if activeTab === 'pals'}
					<div class="btn-group bg-surface-900 w-full items-center rounded-sm p-1">
						<Tooltip position="right" label="Add Pal to Base">
							<button
								class="btn hover:bg-secondary-500/50 p-2"
								onclick={() => currentBase && handleAddPal(currentBase[0])}
							>
								<Plus />
							</button>
						</Tooltip>
						<Tooltip label="Select all in current base">
							<button class="btn hover:bg-secondary-500/50 p-2" onclick={handleSelectAll}>
								<ReplaceAll />
							</button>
						</Tooltip>
						<Tooltip label="Heal all in current base">
							<button class="btn hover:bg-secondary-500/50 p-2" onclick={handleHealAll}>
								<Bandage />
							</button>
						</Tooltip>
						{#if selectedPals.length > 0}
							<Tooltip label="Apply preset to selected pal(s)">
								<button class="btn hover:bg-secondary-500/50 p-2" onclick={handleSelectPreset}>
									<Play />
								</button>
							</Tooltip>
							<Tooltip label="Heal selected pal(s)">
								<button class="btn hover:bg-secondary-500/50 p-2" onclick={healSelectedPals}>
									<Ambulance />
								</button>
							</Tooltip>
							<Tooltip label="Delete selected pal(s)">
								<button class="btn hover:bg-secondary-500/50 p-2" onclick={deleteSelectedPals}>
									<Trash />
								</button>
							</Tooltip>
							<Tooltip label="Clear selected">
								<button
									class="btn hover:bg-secondary-500/50 p-2"
									onclick={() => (selectedPals = [])}
								>
									<X />
								</button>
							</Tooltip>
						{/if}
					</div>
				{/if}
				{#if activeTab == 'storage'}
					<div class="flex items-center">
						<Input bind:value={inventorySearchQuery} placeholder="Search Inventory" />
						<button
							class="btn"
							onclick={() => {
								inventorySearchQuery = '';
								selectedInventoryItem = '';
							}}
						>
							<RefreshCcw class="h-6 w-6" />
						</button>
					</div>
					<List
						bind:items={currentBaseInventory.current}
						baseClass="w-full"
						listClass="h-[380px] 2xl:h-[630px]"
						canSelect={false}
						idKey="static_id"
						headerClass="grid w-full grid-cols-[auto_1fr_auto] gap-2 rounded-sm"
						onselect={(item) => {
							selectedInventoryItem = item.static_id;
							inventorySearchQuery = '';
						}}
					>
						{#snippet listHeader()}
							<div class="h-8 w-8"></div>
							<span class="font-bold">Inventory</span>
							<span class="font-bold">Total</span>
						{/snippet}
						{#snippet listItem(item)}
							{@const itemData = itemsData.items[fixStupidTypos(item.static_id)]}
							{#if itemData}
								{@const itemIcon = assetLoader.loadImage(
									`${ASSET_DATA_PATH}/img/${itemData.details.icon}.png`
								)}
								<div class="grid w-full grid-cols-[auto_1fr_auto] gap-2">
									<div class={getItemBackground(itemData.details.rarity)}>
										<img
											src={itemIcon || staticIcons.unknownIcon}
											alt={itemData.info.localized_name}
											class="h-8 w-8"
										/>
									</div>
									<span>{itemData.info.localized_name}</span>
									<span>{item.total_count.toLocaleString()}</span>
								</div>
							{:else}
								<div class="grid w-full grid-cols-[auto_1fr_auto] gap-2">
									<img src={staticIcons.unknownIcon} alt={item.static_id} class="h-8 w-8" />
									<span>{item.static_id}</span>
									<span>{item.total_count.toLocaleString()}</span>
								</div>
							{/if}
						{/snippet}
						{#snippet listItemPopup(item)}
							{@const itemData = itemsData.items[item.static_id]}
							{#if itemData}
								<div class="flex flex-col">
									<span class="font-bold">{itemData.info.localized_name}</span>
									<span class="text-sm">{itemData.info.description}</span>
									<hr class="border-surface-500 my-2" />
									<span class="font-bold">Total Count: {item.total_count}</span>
									{#each Object.entries(item.containers) as [containerId, count]}
										{@const building = buildingsData.buildings[fixStupidTypos(containerId)]}
										{#if building}
											{@const buildingIcon = assetLoader.loadImage(
												`${ASSET_DATA_PATH}/img/${building.icon}.png`
											)}
											<div class="grid w-full grid-cols-[auto_1fr_auto] gap-2">
												<img
													src={buildingIcon || staticIcons.unknownIcon}
													alt={building.localized_name}
													class="h-8 w-8"
												/>
												<span>{building.localized_name}</span>
												<span>{count.toLocaleString()}</span>
											</div>
										{:else if !ignoreKeys.some((key) => containerId.includes(key))}
											<div class="grid w-full grid-cols-2 gap-2">
												<span class="font-bold"> {containerId}: </span>
												<span>{count.toLocaleString()}</span>
											</div>
										{/if}
									{/each}
								</div>
							{:else}
								{item.static_id}
							{/if}
						{/snippet}
					</List>
				{/if}
			</div>

			<!-- Right Content -->
			<div>
				<!-- Pager -->
				<div class="mb-4 flex items-center justify-center space-x-4">
					<button class="rounded-sm px-4 py-2 font-bold" onclick={decrementPage}>
						<img src={staticIcons.qIcon} alt="Previous" class="h-10 w-10" />
					</button>

					<div class="flex space-x-2">
						{#each visiblePages as page}
							<TooltipButton
								class="h-8 w-8 rounded-full {page === currentPage
									? 'bg-primary-500 text-white'
									: 'bg-surface-800 hover:bg-gray-300'}"
								onclick={() => (currentPage = page)}
								popupLabel={`Base ${Object.entries(guildBases!)[page - 1]?.[0]}`}
							>
								{page}
							</TooltipButton>
						{/each}
					</div>

					<button class="rounded-sm px-4 py-2 font-bold" onclick={incrementPage}>
						<img src={staticIcons.eIcon} alt="Next" class="h-10 w-10" />
					</button>
				</div>
				{#if activeTab == 'pals'}
					<div class="overflow-hidden">
						<div class="grid grid-cols-6 place-items-center gap-4 p-4">
							{#each currentPageItems as item (item.pal.instance_id)}
								{#if item.pal.character_id !== 'None' || !palSearchQuery}
									<PalBadge
										bind:pal={item.pal}
										bind:selected={selectedPals}
										onSelect={handlePalSelect}
										onDelete={() => handleDeletePal(currentBase![0], item.pal)}
										onAdd={() => handleAddPal(currentBase![0], item.pal.storage_slot)}
										onClone={() => handleClonePal(item)}
										onMove={() => {}}
									/>
								{/if}
							{/each}
						</div>
					</div>
				{:else if activeTab == 'storage'}
					{#if currentBaseStorageContainers && currentBaseStorageContainers.length > 0}
						<div class="flex space-x-4">
							<List
								items={currentBaseStorageContainers}
								baseClass="w-1/4"
								listClass="h-[550px] 2xl:h-[800px]"
								canSelect={false}
								onselect={(itemContainer) => handleSelectStorageContainer(itemContainer)}
							>
								{#snippet listItem(item)}
									{@const building = buildingsData.buildings[fixStupidTypos(item.key)]}
									{#if building}
										{@const buildingIcon = assetLoader.loadImage(
											`${ASSET_DATA_PATH}/img/${building.icon}.png`
										)}
										<div class="grid grid-cols-[auto_1fr] gap-2">
											<img
												src={buildingIcon || staticIcons.unknownIcon}
												alt={building.localized_name}
												class="h-8 w-8"
											/>
											<span>{building.localized_name}</span>
										</div>
									{:else}
										<div class="grid grid-cols-[auto_1fr] gap-2">
											<img src={staticIcons.unknownIcon} alt={item.key} class="h-8 w-8" />
											<span>{item.key}</span>
										</div>
									{/if}
								{/snippet}
								{#snippet listItemPopup(item)}
									{@const building = buildingsData.buildings[fixStupidTypos(item.key)]}
									{#if building}
										<div class="flex flex-col">
											<h4 class="h4">{building.localized_name}</h4>
											<div class="grid w-full grid-cols-2 gap-2">
												<span class="font-bold"> Available Slots: </span>
												<span>{item.slot_num}</span>
											</div>
											<div class="grid w-full grid-cols-2 gap-2">
												<span class="font-bold"> Used Slots: </span>
												<span>
													{item?.slots?.filter((slot) => slot.static_id !== 'None').length}
												</span>
											</div>
										</div>
									{:else}
										{item.key}
									{/if}
								{/snippet}
							</List>
							<div class="max-h-[550px] overflow-y-auto 2xl:max-h-[800px]">
								{#if currentStorageContainer}
									{@const building = buildingsData.buildings[currentStorageContainer.key]}
									{@const itemGroup = building?.type_a == BuildingTypeA.Food ? 'Food' : 'Common'}
									<div class="flex items-start space-x-4">
										<div class="m-1 grid grid-cols-6 gap-2">
											{#each Object.values(currentStorageContainer.slots) as _, index}
												<ItemBadge
													bind:slot={currentStorageContainer.slots[index]}
													{itemGroup}
													onUpdate={() => {
														currentStorageContainer!.state = EntryState.MODIFIED;
													}}
													onCopyPaste={(event) => {
														handleCopyPaste(event, currentStorageContainer!.slots[index], true);
														currentStorageContainer!.state = EntryState.MODIFIED;
													}}
												/>
											{/each}
										</div>
										{#if currentStorageContainerIcon}
											<div class="ml-2 flex flex-col">
												<img
													src={currentStorageContainerIcon}
													alt="Storage Container Icon"
													class="h-48 w-48 2xl:h-64 2xl:w-64"
												/>
												<StoragePresets
													container={currentStorageContainer}
													onUpdate={() => {
														currentStorageContainer!.state = EntryState.MODIFIED;
													}}
												/>
											</div>
										{/if}
									</div>
								{:else}
									<div class="flex w-full items-center justify-center">
										<h2 class="h2">Select a Storage Container</h2>
									</div>
								{/if}
							</div>
						</div>
					{:else}
						<div class="flex w-full items-center justify-center">
							<h2 class="h2">No Storage Containers</h2>
						</div>
					{/if}
				{:else if activeTab == 'guildChest' && playerGuild?.guild_chest}
					{@const building = buildingsData.buildings['GuildChest']}
					{@const itemGroup = building?.type_a == BuildingTypeA.Food ? 'Food' : 'Common'}
					<div class="max-h-[550px] overflow-y-auto 2xl:max-h-[800px]">
						<div class="flex items-start space-x-4">
							<div class="m-1 grid grid-cols-6 gap-2">
								{#each Object.values(playerGuild.guild_chest.slots) as _, index}
									<ItemBadge
										bind:slot={playerGuild.guild_chest.slots[index]}
										{itemGroup}
										onUpdate={() => {
											playerGuild.guild_chest!.state = EntryState.MODIFIED;
										}}
										onCopyPaste={(event) => {
											handleCopyPaste(event, playerGuild.guild_chest!.slots[index], true);
											playerGuild.guild_chest!.state = EntryState.MODIFIED;
										}}
									/>
								{/each}
							</div>
							{#if guildChestIcon}
								<div class="flex flex-col">
									<img
										src={guildChestIcon}
										alt="Storage Container Icon"
										class="ml-8 h-48 w-48 2xl:h-64 2xl:w-64"
									/>
									<StoragePresets
										container={playerGuild.guild_chest}
										onUpdate={() => {
											playerGuild.guild_chest!.state = EntryState.MODIFIED;
										}}
									/>
								</div>
							{/if}
						</div>
					</div>
				{/if}
			</div>
		</div>
	{/if}
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">Select a Player to view Guilds</h2>
	</div>
{/if}
