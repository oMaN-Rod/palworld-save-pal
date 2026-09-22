<script lang="ts">
	import { SvelteSet } from 'svelte/reactivity';

	import { palsData, itemsData, presetsData } from '$lib/data';
	import { getAppState, getModalState, getPalEditorState, getToastState } from '$states';
	import { Input, Spinner } from '$components/ui';
	import { ActionGroup, type ActionDescriptor } from '$components/ui/actions';
	import {
		type ItemContainer,
		type Pal,
		type ItemContainerSlot,
		MessageType,
		EntryState
	} from '$types';
	import { PalBadge } from '$components/pal';
	import { PalContainerView } from '$components/pal/container';
	import LabResearch from '$components/guilds/LabResearch.svelte';
	import {
		PalSelectModal,
		NumberInputModal,
		PalPresetSelectModal,
		NumberSliderModal,
		TextInputModal
	} from '$components/modals';
	import { deepCopy, formatBossCharacterId, formatNickname } from '$utils';
	import { cn } from '$theme';
	import { staticIcons } from '$types/icons';
	import type { PalContainerSelection } from '$states/palContainer.svelte';
	import { send } from '$lib/utils/websocketUtils';
	import { goto } from '$app/navigation';
	import { LabResearchControls } from '$components/guilds';
	import * as m from '$i18n/messages';
	import { c, p } from '$lib/utils/commonTranslations';

	import GuildBasePager from './components/GuildBasePager.svelte';
	import GuildHeader from './components/GuildHeader.svelte';
	import GuildInventoryPanel from './components/GuildInventoryPanel.svelte';
	import GuildChest from './components/GuildChest.svelte';
	import GuildStorage from './components/GuildStorage.svelte';
	import { IGNORED_CONTAINER_KEYS, type GuildInventoryItem } from './guildStorage';
	import { buildGuildActions } from './guildActions';

	interface PalWithBaseId {
		pal: Pal;
		baseId: string;
	}

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();
	const palEditor = getPalEditorState();

	// SvelteSet: the view re-reads the selection on mutation, not replacement.
	const selectedIds = new SvelteSet<string>();
	const selectedIdList = $derived([...selectedIds]);

	let palSearchQuery = $state('');
	let baseNumber = $state(1);
	let activeTab: 'pals' | 'storage' | 'guildChest' | 'lab' = $state('pals');
	let currentStorageContainer: (ItemContainer & { slots: ItemContainerSlot[] }) | undefined =
		$state(undefined);
	let selectedInventoryItem: string = $state('');
	let inventorySearchQuery: string = $state('');
	let selectedCategory: string = $state('Handcraft');
	let labResearchComponent: any = $state(null);

	const playerGuild = $derived.by(() => {
		if (appState.selectedPlayer?.guild_id) {
			return appState.guilds[appState.selectedPlayer.guild_id];
		}
	});

	const guildBases = $derived.by(() => {
		if (playerGuild) {
			return playerGuild.bases;
		}
	});

	const baseCount = $derived(Object.keys(guildBases || {}).length);

	const currentBase = $derived.by(() => {
		if (!guildBases) return null;
		const baseEntries = Object.entries(guildBases);
		return baseEntries[baseNumber - 1] || null;
	});

	const currentBaseStorageContainers = $derived.by(() => {
		if (!currentBase) return null;
		const [_, base] = currentBase;
		return Object.values(base.storage_containers)
			.filter(
				(container) =>
					container.slot_num !== 0 &&
					!IGNORED_CONTAINER_KEYS.some((key) => container.key.includes(key))
			)
			.filter(
				(container) =>
					(container.slots.some((s) => {
						const itemData = itemsData.getByKey(s.static_id);
						return (
							s.static_id.toLowerCase().includes(selectedInventoryItem.toLowerCase()) ||
							(itemData &&
								itemData.info.localized_name
									.toLowerCase()
									.includes(selectedInventoryItem.toLowerCase()))
						);
					}) &&
						container.slots.some((s) => {
							const itemData = itemsData.getByKey(s.static_id);
							return (
								s.static_id.toLowerCase().includes(inventorySearchQuery.toLowerCase()) ||
								(itemData &&
									itemData.info.localized_name
										.toLowerCase()
										.includes(inventorySearchQuery.toLowerCase()))
							);
						})) ||
					container.slots.every((s) => s.static_id === 'None')
			)
			.sort((a, b) => a.key.localeCompare(b.key));
	});

	const currentBaseInventory = $derived.by(() => {
		if (!currentBase) return { current: [] };
		const [_, base] = currentBase;
		let inventoryItems: Record<string, Omit<GuildInventoryItem, 'static_id'>> = {};
		for (const container of Object.values(currentBaseStorageContainers || {})) {
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
				const itemData = itemsData.getByKey(static_id);
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
				const itemA = itemsData.getByKey(a.static_id);
				const itemB = itemsData.getByKey(b.static_id);
				if (itemA && itemB) {
					return itemA.info.localized_name.localeCompare(itemB.info.localized_name);
				}
				return a.static_id.localeCompare(b.static_id);
			});
		return {
			current: items
		};
	});

	const matchingPals = $derived.by((): PalWithBaseId[] => {
		if (!guildBases || !palSearchQuery) return [];
		const query = palSearchQuery.toLowerCase();
		return Object.entries(guildBases).flatMap(([baseId, base]) =>
			Object.values(base.pals)
				.filter(
					(pal) =>
						pal.character_id !== 'None' &&
						(pal.name.toLowerCase().includes(query) ||
							pal.nickname?.toLowerCase().includes(query) ||
							pal.character_id.toLowerCase().includes(query))
				)
				.map((pal) => ({ pal, baseId }))
		);
	});

	// A search spans every base, so empty slots (which belong to one base) are dropped.
	const displayPals = $derived.by((): PalWithBaseId[] => {
		if (palSearchQuery) return matchingPals;
		if (!currentBase) return [];
		const [baseId, base] = currentBase;

		const palsBySlot = new Map<number, (typeof base.pals)[string]>();
		for (const pal of Object.values(base.pals)) {
			if (!palsBySlot.has(pal.storage_slot)) palsBySlot.set(pal.storage_slot, pal);
		}

		return Array(base.slot_count)
			.fill(undefined)
			.map((_, index) => {
				const existingPal = palsBySlot.get(index);
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

	// An empty slot has no record, so no bulk operation could resolve its id.
	const selectablePals = $derived(
		new Map(displayPals.filter(isRealPal).map((item) => [item.pal.instance_id, item]))
	);

	const selection: PalContainerSelection<string> = {
		get ids() {
			return selectedIds;
		},
		onToggle: (id: string) => toggleSelected(id)
	};

	const guildActions = $derived(
		buildGuildActions({
			selectionCount: selectedIds.size,
			baseId: currentBase?.[0] ?? '',
			addPal: handleAddPal,
			selectAll: handleSelectAll,
			healAll: handleHealAll,
			applyPreset: handleSelectPreset,
			healSelected: healSelectedPals,
			deleteSelected: deleteSelectedPals,
			clearSelection: () => selectedIds.clear()
		})
	);

	function handleKeydown(event: KeyboardEvent) {
		if (event.target instanceof HTMLInputElement) return;

		if (event.key === 'ArrowLeft' || event.key === 'q' || event.key === 'Q') {
			previousBase();
		} else if (event.key === 'ArrowRight' || event.key === 'e' || event.key === 'E') {
			nextBase();
		}
	}

	function selectBase(base: number) {
		baseNumber = base;
	}

	function clearBaseStorageView() {
		currentStorageContainer = undefined;
		inventorySearchQuery = '';
		selectedInventoryItem = '';
	}

	function previousBase() {
		selectBase(baseNumber > 1 ? baseNumber - 1 : baseCount);
		clearBaseStorageView();
	}

	function nextBase() {
		selectBase(baseNumber < baseCount ? baseNumber + 1 : 1);
		clearBaseStorageView();
	}

	function isRealPal(item: PalWithBaseId): boolean {
		return item.pal.character_id !== 'None';
	}

	function nicknameOf(item: PalWithBaseId): string {
		return item.pal.nickname || item.pal.name || item.pal.character_id;
	}

	function toggleSelected(id: string): void {
		if (!selectablePals.has(id)) return;
		if (selectedIds.has(id)) {
			selectedIds.delete(id);
		} else {
			selectedIds.add(id);
		}
	}

	function handleOpenPal(item: PalWithBaseId): void {
		if (!isRealPal(item)) {
			handleAddPal(item.baseId, item.pal.storage_slot);
			return;
		}
		palEditor.open(item.pal);
	}

	// The base comes off the item: a search result may live outside `currentBase`.
	function palActions(item: PalWithBaseId): ActionDescriptor[] {
		if (!isRealPal(item)) {
			return [
				{
					id: 'guild-pal-add',
					label: m.add_new_pal(p.pal),
					icon: 'tabler:plus',
					run: () => handleAddPal(item.baseId, item.pal.storage_slot)
				}
			];
		}

		return [
			{
				id: 'guild-pal-clone',
				label: m.clone_selected_pal(p.pal),
				icon: 'tabler:copy',
				run: () => handleClonePal(item)
			},
			{
				id: 'guild-pal-delete',
				label: m.delete_entity({ entity: c.pal }),
				icon: 'tabler:trash',
				run: () => handleDeletePal(item.baseId, item.pal),
				danger: true
			}
		];
	}

	async function handleAddPal(baseId: string, index?: number) {
		if (!appState.selectedPlayer || !guildBases) return;
		const base = guildBases[baseId];
		if (!base) return;

		// @ts-ignore
		const result = await modal.showModal<[string, string] | undefined>(PalSelectModal, {
			title: m.add_pal_to_base({ pal: c.pal, base: baseNumber })
		});
		if (!result) return;

		const [selectedPal, nickname] = result;
		const palData = palsData.getByKey(selectedPal);

		send(MessageType.ADD_PAL, {
			guild_id: playerGuild?.id,
			base_id: baseId,
			character_id: selectedPal,
			nickname:
				nickname ||
				formatNickname(palData?.localized_name ?? selectedPal, appState.settings.new_pal_prefix),
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
			toast.add(m.no_slots_available_in_entity({ entity: c.base }), m.error(), 'error');
			return;
		}

		// @ts-ignore
		const result = await modal.showModal<number>(NumberInputModal, {
			title: m.how_many_clones(),
			message: m.slots_available_in_entity({ count: maxClones, entity: c.base }),
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
		if (selectedIds.size === 0) return;

		const count = selectedIds.size;
		const confirmed = await modal.showConfirmModal({
			title: m.delete_entity({ entity: m.pal({ count }) }),
			message: m.delete_count_entities_confirm({
				count,
				entity: m.pal({ count })
			}),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});

		if (confirmed) {
			const baseId = currentBase ? currentBase[0] : '';
			send(MessageType.DELETE_PALS, {
				guild_id: playerGuild?.id,
				base_id: baseId,
				pal_ids: selectedIdList
			});

			playerGuild!.bases[baseId].pals = Object.fromEntries(
				Object.entries(playerGuild!.bases[baseId].pals).filter(([id]) => !selectedIds.has(id))
			);
		}

		selectedIds.clear();
	}

	async function handleDeletePal(baseId: string, pal: Pal) {
		const confirmed = await modal.showConfirmModal({
			title: m.delete_entity({ entity: c.pal }),
			message: m.delete_entity_by_name_confirm({ name: pal.nickname || pal.name }),
			confirmText: m.delete(),
			cancelText: m.cancel()
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

	function handleSelectAll() {
		if (!currentBase) return;
		const [, base] = currentBase;

		const basePalIds = Object.values(base.pals).map((pal) => pal.instance_id);
		// A count comparison, not a subset test, so a second press clears.
		const wasComplete = selectedIds.size === basePalIds.length;

		selectedIds.clear();
		if (wasComplete) return;
		for (const id of basePalIds) selectedIds.add(id);
	}

	async function healSelectedPals() {
		if (!guildBases || selectedIds.size === 0) return;
		send(MessageType.HEAL_PALS, selectedIdList);

		Object.values(guildBases).forEach((base) => {
			Object.values(base.pals).forEach((pal) => {
				if (selectedIds.has(pal.instance_id)) {
					pal.hp = pal.max_hp;
					pal.sanity = 100;
					const palData = palsData.getByKey(pal.character_key);
					if (palData) {
						pal.stomach = palData.max_full_stomach;
					}
				}
			});
		});

		selectedIds.clear();
	}

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
				const palData = palsData.getByKey(pal.character_key);
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
			toast.add(m.cannot_paste_here(), undefined, 'warning');
		}
	}

	async function handleSelectPreset() {
		const selectedPalsData = selectedIdList.map((id) => {
			const pal = Object.values(currentBase![1].pals).find((p) => p.instance_id === id);
			return {
				character_id: pal?.character_id,
				character_key: pal?.character_key
			};
		});
		// @ts-ignore
		const result = await modal.showModal<string>(PalPresetSelectModal, {
			title: m.select_entity({ entity: c.preset }),
			selectedPals: selectedPalsData
		});
		if (!result) return;

		const presetProfile = presetsData.presetProfiles[result];

		selectedIdList.forEach((id) => {
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
				formatBossCharacterId(pal);
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

	async function handleEditBaseName() {
		if (!currentBase) return;
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: m.edit_entity({ entity: m.base_name() }),
			value: currentBase[1].name || ''
		});
		if (!result) return;
		currentBase[1].name = result;
		playerGuild!.state = EntryState.MODIFIED;
	}

	async function handleEditGuildName() {
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: m.edit_entity({ entity: m.guild_name() }),
			value: playerGuild!.name
		});
		if (!result) return;
		playerGuild!.name = result;
		playerGuild!.state = EntryState.MODIFIED;
	}

	async function handleEditBasecampLevel() {
		// @ts-ignore
		const result = await modal.showModal<number>(NumberSliderModal, {
			title: m.edit_entity({ entity: m.basecamp_level() }),
			value: playerGuild!.base_camp_level || 1,
			min: 1,
			max: 30,
			markers: [5, 10, 15, 20, 25, 30]
		});
		if (!result) return;
		playerGuild!.base_camp_level = result;
		playerGuild!.state = EntryState.MODIFIED;
	}

	async function handleDeleteGuild() {
		const confirmed = await modal.showConfirmModal({
			title: m.delete_entity({ entity: c.guild }),
			message: m.delete_entity_by_name_confirm({ name: playerGuild!.name }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});
		if (confirmed) {
			send(MessageType.DELETE_GUILD, {
				guild_id: playerGuild?.id,
				origin: 'edit'
			});
			goto('/loading');
		}
	}

	$effect(() => {
		window.addEventListener('keydown', handleKeydown);
		return () => {
			window.removeEventListener('keydown', handleKeydown);
		};
	});

	$effect(() => {
		if (baseNumber > baseCount && baseCount > 0) {
			baseNumber = baseCount;
		}
	});

	$effect(() => {
		if (inventorySearchQuery !== '') {
			selectedInventoryItem = '';
		}
	});
</script>

<!-- The page's own search box: its filtering spans every base, which `matches` can't express. -->
{#snippet filters()}
	<div id="guild-pal-filters" class="flex flex-col gap-4">
		<Input
			type="text"
			inputClass="w-full"
			placeholder={m.search_by_name_nickname()}
			bind:value={palSearchQuery}
		/>
	</div>
{/snippet}

{#snippet portrait(item: PalWithBaseId)}
	<PalBadge
		pal={item.pal}
		selected={selectedIdList}
		onDelete={() => handleDeletePal(item.baseId, item.pal)}
		onAdd={() => handleAddPal(item.baseId, item.pal.storage_slot)}
		onClone={() => handleClonePal(item)}
	/>
{/snippet}

{#snippet columns(item: PalWithBaseId)}
	{#if isRealPal(item)}
		<div class="flex flex-col gap-0.5">
			<span class="truncate text-sm font-bold">{nicknameOf(item)}</span>
			<span class="text-surface-400 text-xs">
				{m.level()}
				{item.pal.level ?? 0}
			</span>
		</div>
	{:else}
		<span class="text-surface-500 text-sm">{m.empty()} · {item.pal.storage_slot + 1}</span>
	{/if}
{/snippet}

{#snippet detail(item: PalWithBaseId)}
	<dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
		<dt class="text-surface-400">{m.nickname()}</dt>
		<dd>{nicknameOf(item)}</dd>
		<dt class="text-surface-400">{m.level()}</dt>
		<dd>{item.pal.level ?? 0}</dd>
		<dt class="text-surface-400">{m.hp()}</dt>
		<dd>{item.pal.hp ?? 0} / {item.pal.max_hp ?? 0}</dd>
	</dl>
{/snippet}

{#if appState.selectedPlayer}
	{#if appState.loadingGuild}
		<div class="flex h-full w-full items-center justify-center">
			<div class="flex flex-col items-center gap-4">
				<Spinner size="size-16" />
				<p class="text-surface-400">{m.loaded_entity({ entity: c.guild })}</p>
			</div>
		</div>
	{:else if !playerGuild}
		<div class="flex w-full items-center justify-center">
			<h2 class="h2">{m.no_guild_found()}</h2>
		</div>
	{:else if !guildBases || Object.values(guildBases).length === 0}
		<div class="flex w-full items-center justify-center space-x-4">
			<h2 class="h2">{m.no_guild_bases_found()}</h2>
			<img src={staticIcons.sadIcon} alt="Sad" class="h-18 w-18" />
		</div>
	{:else}
		<div class="grid h-full w-full grid-cols-[minmax(200px,25%)_1fr] xl:grid-cols-[25%_1fr]">
			<div class="shrink-0 space-y-2 p-4">
				<GuildHeader
					guild={playerGuild}
					base={currentBase?.[1] ?? null}
					{baseNumber}
					debugMode={appState.settings.debug_mode}
					onEditGuildName={handleEditGuildName}
					onEditBasecampLevel={handleEditBasecampLevel}
					onDeleteGuild={handleDeleteGuild}
					onEditBaseName={handleEditBaseName}
				/>

				<nav
					id="guild-tabs"
					class="btn-group preset-outlined-surface-200-800 w-full flex-col rounded-sm p-2 md:flex-row"
				>
					<button
						id="guild-tab-pals"
						class={cn(
							'btn hover:bg-secondary-500/50 w-1/4 rounded-sm',
							activeTab == 'pals' ? 'bg-secondary-800 text-white' : ''
						)}
						onclick={() => {
							activeTab = 'pals';
							inventorySearchQuery = '';
							selectedInventoryItem = '';
						}}
					>
						<span>{c.pals}</span>
					</button>
					<button
						id="guild-tab-storage"
						class={cn(
							'btn hover:bg-secondary-500/50 w-1/4 rounded-sm',
							activeTab == 'storage' ? 'bg-secondary-800 text-white' : ''
						)}
						onclick={() => {
							activeTab = 'storage';
							inventorySearchQuery = '';
							selectedInventoryItem = '';
						}}
					>
						<span>{m.storage()}</span>
					</button>
					<button
						id="guild-tab-chest"
						class={cn(
							'btn hover:bg-secondary-500/50 w-1/4 rounded-sm',
							activeTab == 'guildChest' ? 'bg-secondary-800 text-white' : ''
						)}
						onclick={() => {
							inventorySearchQuery = '';
							selectedInventoryItem = '';
							handleSelectGuildChest();
						}}
					>
						<span>{m.chest()}</span>
					</button>
					<button
						id="guild-tab-lab"
						class={cn(
							'btn hover:bg-secondary-500/50 w-1/4 rounded-sm',
							activeTab == 'lab' ? 'bg-secondary-800 text-white' : ''
						)}
						onclick={() => {
							inventorySearchQuery = '';
							selectedInventoryItem = '';
							activeTab = 'lab';
						}}
					>
						<span>{m.lab()}</span>
					</button>
				</nav>
				{#if activeTab === 'pals' && selectedIds.size === 0}
					<!-- The view's own toolbar carries these rows once something is selected. -->
					<ActionGroup id="guild-pals-actions" actions={guildActions} title={m.quick_actions()} />
				{/if}
				{#if activeTab == 'storage'}
					<GuildInventoryPanel
						items={currentBaseInventory.current}
						bind:searchQuery={inventorySearchQuery}
						onSelect={(staticId) => {
							selectedInventoryItem = staticId;
							inventorySearchQuery = '';
						}}
						onReset={() => {
							inventorySearchQuery = '';
							selectedInventoryItem = '';
						}}
					/>
				{/if}
				{#if activeTab === 'lab'}
					<LabResearchControls
						bind:selectedCategory
						guild={playerGuild}
						unlockAllForCategory={labResearchComponent?.unlockAllForCategory}
					/>
				{/if}
			</div>

			<div>
				{#if activeTab !== 'lab'}
					<GuildBasePager
						total={baseCount}
						current={baseNumber}
						onSelect={selectBase}
						onPrevious={previousBase}
						onNext={nextBase}
					/>
				{/if}
				{#if activeTab == 'pals'}
					<!-- `pageSize={0}` disables paging: guild pages bases, not pals. -->
					<div class="min-h-0">
						<PalContainerView
							pals={displayPals}
							idOf={(item) => item.pal.instance_id}
							{nicknameOf}
							storageKey="guild"
							title={c.pals}
							pageSize={0}
							actions={guildActions}
							{selection}
							{filters}
							{portrait}
							{columns}
							{detail}
							{palActions}
							onOpenPal={handleOpenPal}
						/>
					</div>
				{:else if activeTab == 'storage'}
					<GuildStorage
						containers={currentBaseStorageContainers ?? []}
						selected={currentStorageContainer}
						onSelect={handleSelectStorageContainer}
						onUpdate={() => {
							currentStorageContainer!.state = EntryState.MODIFIED;
						}}
						onCopyPaste={(event, slot) => {
							handleCopyPaste(event, slot, true);
							currentStorageContainer!.state = EntryState.MODIFIED;
						}}
					/>
				{:else if activeTab == 'guildChest' && playerGuild?.guild_chest}
					<GuildChest
						chest={playerGuild.guild_chest as ItemContainer & { slots: ItemContainerSlot[] }}
						onUpdate={() => {
							playerGuild.guild_chest!.state = EntryState.MODIFIED;
						}}
						onCopyPaste={(event, slot) => {
							handleCopyPaste(event, slot, true);
							playerGuild.guild_chest!.state = EntryState.MODIFIED;
						}}
					/>
				{:else if activeTab == 'lab'}
					<div id="guild-lab-content" class="h-full w-full">
						<LabResearch
							bind:this={labResearchComponent}
							guild={playerGuild}
							bind:selectedCategory
						/>
					</div>
				{/if}
			</div>
		</div>
	{/if}
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">{m.select_player_view_entity({ entity: c.guild })}</h2>
	</div>
{/if}
