<script lang="ts">
	import { SvelteSet } from 'svelte/reactivity';

	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { elementsData, palsData, presetsData } from '$lib/data';
	import {
		getAppState,
		getModalState,
		getPalEditorState,
		getToastState,
		getUpsState
	} from '$states';
	import { Card, Input, Tooltip } from '$components/ui';
	import { ActionGroup, type ActionDescriptor } from '$components/ui/actions';
	import {
		NumberInputModal,
		PalSelectModal,
		PalPresetSelectModal,
		FillPalsModal,
		CloneToUpsModal
	} from '$components/modals';
	import { type Pal, type PalData, MessageType, type CloneToUpsModalProps } from '$types';
	import {
		deepCopy,
		handleMaxOutPal,
		formatNickname,
		applyPalPreset,
		palMatchesFilter
	} from '$utils';
	import { cn } from '$theme';
	import { PalCard, PalBadge, PalContainerStats, PalFilterButtons } from '$components/pal';
	import { PalContainerView } from '$components/pal/container';
	import type { PalContainerSelection } from '$states/palContainer.svelte';
	import { send } from '$lib/utils/websocketUtils';
	import * as m from '$i18n/messages';
	import { c, p } from '$lib/utils/commonTranslations';

	import { buildPalboxActions } from './palboxActions';

	const PALS_PER_PAGE = 30;
	const TOTAL_SLOTS = 960;

	type SortBy = 'name' | 'level' | 'paldeck-index' | 'slot-index';
	type SortOrder = 'asc' | 'desc';

	type PalWithData = {
		id: string;
		pal: Pal;
		palData?: PalData;
	};

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();
	const upsState = getUpsState();
	const palEditor = getPalEditorState();

	let { ...additionalProps } = $props<{
		[key: string]: any;
	}>();

	let searchQuery = $state('');
	let selectedFilter = $state('All');
	let sortBy: SortBy = $state('slot-index');
	let sortOrder: SortOrder = $state('asc');

	// SvelteSet: the view re-reads the selection on mutation, not replacement.
	const selectedIds = new SvelteSet<string>();
	const selectedIdList = $derived([...selectedIds]);

	const otomoContainer: Record<string, Pal> = $derived.by(() => {
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
						return [emptyPalId, { character_key: 'None' }];
					}
				});

			return Object.fromEntries(allSlots);
		}
		return {};
	});

	const boxPals = $derived.by((): PalWithData[] => {
		const player = appState.selectedPlayer;
		if (!player || !player.pals) return [];
		const palBoxId = player.pal_box_id;
		return Object.entries(player.pals as Record<string, Pal>)
			.filter(([_, pal]) => pal.storage_id === palBoxId)
			.map(([id, pal]) => ({ id, pal, palData: palsData.getByKey(pal.character_key) }));
	});

	const knownPals = $derived(boxPals.filter(({ palData }) => Boolean(palData)));

	const matchingPals = $derived.by(() => {
		const query = searchQuery.toLowerCase();
		return knownPals.filter(({ pal, palData }) => {
			const matchesSearch =
				query === '' ||
				Boolean(pal.name?.toLowerCase().includes(query)) ||
				Boolean(pal.nickname?.toLowerCase().includes(query)) ||
				Boolean(pal.character_id?.toLowerCase().includes(query));
			return matchesSearch && palMatchesFilter(pal, palData as PalData, selectedFilter);
		});
	});

	const sortedPals = $derived.by(() => {
		const direction = sortOrder === 'asc' ? 1 : -1;
		const list = [...matchingPals];
		switch (sortBy) {
			case 'name':
				return list.sort((a, b) => direction * (a.pal.name ?? '').localeCompare(b.pal.name ?? ''));
			case 'level':
				return list.sort((a, b) => direction * (a.pal.level - b.pal.level));
			case 'paldeck-index':
				return list.sort((a, b) => {
					const indexA = a.palData?.pal_deck_index ?? Infinity;
					const indexB = b.palData?.pal_deck_index ?? Infinity;
					if (indexA === indexB) return 0;
					return direction * (indexA - indexB);
				});
			default:
				return list.sort((a, b) => direction * (a.pal.storage_slot - b.pal.storage_slot));
		}
	});

	// Once the list is narrowed or reordered, slot numbers mean nothing.
	const showsEmptySlots = $derived(
		searchQuery === '' && selectedFilter === 'All' && sortBy === 'slot-index'
	);

	const displayPals = $derived.by((): PalWithData[] => {
		if (!showsEmptySlots) return sortedPals;

		const bySlot = new Map<number, PalWithData>();
		for (const entry of sortedPals) {
			if (!bySlot.has(entry.pal.storage_slot)) {
				bySlot.set(entry.pal.storage_slot, entry);
			}
		}

		return Array.from({ length: TOTAL_SLOTS }, (_, index) => bySlot.get(index) ?? emptySlot(index));
	});

	const elementTypes = $derived(Object.keys(elementsData.elements));

	const selection: PalContainerSelection<string> = {
		get ids() {
			return selectedIds;
		},
		onToggle: (id: string) => toggleSelected(id)
	};

	const palboxActions = $derived(
		buildPalboxActions({
			selectionCount: selectedIds.size,
			addPal: () => handleAddPal('palbox'),
			addAllPals: addAllPalsToBox,
			selectAll,
			healAll: handleHealAll,
			cloneSelected: cloneSelectedPal,
			applyPreset: handleSelectPreset,
			cloneSelectedToUps: handleBulkCloneToUps,
			healSelected: healSelectedPals,
			maxSelected: maxSelectedPals,
			deleteSelected: deleteSelectedPals,
			clearSelection: () => selectedIds.clear()
		})
	);

	const sortButtonClass = (currentSortBy: SortBy) =>
		cn('btn', sortBy === currentSortBy ? 'bg-secondary-500/25' : '');

	const LevelSortIcon = $derived.by(() => {
		if (sortBy !== 'level') {
			return 'tabler:sort-ascending-numbers';
		} else {
			return sortOrder === 'asc'
				? 'tabler:sort-ascending-numbers'
				: 'tabler:sort-descending-numbers';
		}
	});

	const NameSortIcon = $derived.by(() => {
		if (sortBy !== 'name') {
			return 'tabler:sort-ascending-letters';
		} else {
			return sortOrder === 'asc'
				? 'tabler:sort-ascending-letters'
				: 'tabler:sort-descending-letters';
		}
	});

	const PaldeckSortIcon = $derived.by(() => {
		if (sortBy !== 'paldeck-index') {
			return 'tabler:arrows-sort';
		} else {
			return sortOrder === 'asc'
				? 'tabler:sort-ascending-numbers'
				: 'tabler:sort-descending-numbers';
		}
	});

	function emptySlot(index: number): PalWithData {
		return {
			id: `empty-${index}`,
			pal: {
				character_id: 'None',
				character_key: 'None',
				storage_slot: index,
				instance_id: `empty-${index}`,
				storage_id: appState.selectedPlayer?.pal_box_id
			} as Pal
		};
	}

	function nicknameOf(entry: PalWithData): string {
		return entry.pal.nickname || entry.pal.name || entry.pal.character_id;
	}

	function isRealPal(entry: PalWithData): boolean {
		return entry.pal.character_id !== 'None';
	}

	// An empty slot has no record, so no bulk operation could resolve its id.
	function toggleSelected(id: string): void {
		if (!appState.selectedPlayer?.pals?.[id]) return;
		if (selectedIds.has(id)) {
			selectedIds.delete(id);
		} else {
			selectedIds.add(id);
		}
	}

	function handlePalSelect(pal: Pal, event: MouseEvent) {
		if (!pal || pal.character_id === 'None') return;
		if (event.ctrlKey || event.metaKey) {
			toggleSelected(pal.instance_id);
		}
	}

	function selectAll(includeParty: boolean): void {
		const boxIds = displayPals.filter(isRealPal).map((entry) => entry.id);
		const partyIds = includeParty
			? Object.values(otomoContainer)
					.filter((pal) => pal.character_id !== 'None')
					.map((pal) => pal.instance_id)
			: [];
		const everything = [...boxIds, ...partyIds];
		const wasComplete = selectedIds.size === everything.length;

		selectedIds.clear();
		if (wasComplete) return;
		for (const id of everything) selectedIds.add(id);
	}

	function handleOpenPal(entry: PalWithData): void {
		if (!isRealPal(entry)) {
			handleAddPal('palbox', entry.pal.storage_slot);
			return;
		}
		palEditor.open(entry.pal);
	}

	function handleMoveToParty(pal: Pal) {
		if (appState.selectedPlayer) {
			send(MessageType.MOVE_PAL, {
				player_id: appState.selectedPlayer.uid,
				pal_id: pal.instance_id,
				container_id: appState.selectedPlayer.otomo_container_id
			});
		}
	}

	function handleMoveToPalbox(pal: Pal) {
		if (appState.selectedPlayer) {
			send(MessageType.MOVE_PAL, {
				player_id: appState.selectedPlayer.uid,
				pal_id: pal.instance_id,
				container_id: appState.selectedPlayer.pal_box_id
			});
		}
	}

	function toggleSort(newSortBy: SortBy) {
		if (sortBy === newSortBy) {
			if (sortOrder === 'desc') {
				sortBy = 'slot-index';
				sortOrder = 'asc';
			} else {
				sortOrder = sortOrder === 'asc' ? 'desc' : 'asc';
			}
		} else {
			sortBy = newSortBy;
			sortOrder = 'asc';
		}
	}

	async function handleAddPal(target: 'party' | 'palbox', index: number | undefined = undefined) {
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		const result = await modal.showModal<[string, string] | undefined>(PalSelectModal, {
			title: m.add_new_pal_to_entity({ entity: target === 'party' ? m.party() : m.palbox() })
		});
		if (!result) return;
		const [selectedPal, nickname] = result;
		const palData = palsData.getByKey(selectedPal);
		const containerId =
			target === 'party'
				? appState.selectedPlayer.otomo_container_id
				: appState.selectedPlayer.pal_box_id;

		send(MessageType.ADD_PAL, {
			player_id: appState.selectedPlayer.uid,
			character_id: selectedPal,
			nickname:
				nickname ||
				formatNickname(palData?.localized_name || selectedPal, appState.settings.new_pal_prefix),
			container_id: containerId,
			storage_slot: index
		});
	}

	async function clonePal(pal: Pal) {
		const maxClones = appState.selectedPlayer!.pals
			? 965 - Object.values(appState.selectedPlayer!.pals).length
			: 0;
		if (maxClones === 0) {
			toast.add(m.no_slots_available_in_entity({ entity: m.palbox() }), m.error(), 'error');
			return;
		}
		// @ts-ignore
		const result = await modal.showModal<number>(NumberInputModal, {
			title: m.how_many_clones(),
			message: m.slots_available_in_entity({ count: maxClones, entity: m.palbox() }),
			value: 1,
			min: 0,
			max: maxClones
		});
		if (!result) return;
		for (let i = 0; i < result; i++) {
			const clonedPal = deepCopy(pal);
			clonedPal.nickname = formatNickname(
				clonedPal.nickname || clonedPal.name || clonedPal.character_id,
				appState.settings.clone_prefix
			);
			send(MessageType.CLONE_PAL, {
				pal: clonedPal
			});
		}
	}

	async function cloneSelectedPal() {
		if (appState.selectedPlayer && appState.selectedPlayer.pals) {
			const [first] = selectedIdList;
			const pal = first ? appState.selectedPlayer.pals[first] : undefined;
			if (!pal) return;
			await clonePal(pal);
		}
	}

	async function handleClonePal(pal: Pal) {
		await clonePal(pal);
	}

	async function handleCloneToUps(pal: Pal) {
		// @ts-ignore
		const result = await modal.showModal<CloneToUpsModalProps>(CloneToUpsModal, {
			title: m.clone_to_entity({ entity: m.ups() }),
			message: m.clone_pal_to_entity({ pal: c.pal, entity: c.universalPalStorage }),
			pals: [pal]
		});

		if (!result) return;

		const { collectionId, tags, notes } = result;

		try {
			await upsState.cloneToUps(
				[pal.instance_id],
				'pal_box',
				appState.selectedPlayer?.uid,
				collectionId,
				tags.length > 0 ? tags : undefined,
				notes || undefined
			);
		} catch (error) {
			console.error('Clone to UPS failed:', error);
			toast.add(m.clone_to_entity_failed({ entity: c.universalPalStorage }), m.error(), 'error');
		}
	}

	async function handleBulkCloneToUps() {
		if (selectedIds.size === 0) return;

		const ids = [...selectedIds];
		const palsToClone = ids
			.map((id) => appState.selectedPlayer?.pals?.[id])
			.filter(Boolean) as Pal[];

		if (palsToClone.length === 0) return;

		// @ts-ignore
		const result = await modal.showModal<CloneToUpsModalProps>(CloneToUpsModal, {
			title: m.clone_to_entity({ entity: c.universalPalStorage }),
			message: m.clone_pal_to_entity({
				pal: m.pal({ count: palsToClone.length }),
				entity: c.universalPalStorage
			}),
			pals: palsToClone
		});

		if (!result) return;

		const { collectionId, tags, notes } = result;

		try {
			await upsState.cloneToUps(
				ids,
				'pal_box',
				appState.selectedPlayer?.uid,
				collectionId,
				tags.length > 0 ? tags : undefined,
				notes || undefined
			);

			selectedIds.clear();
		} catch (error) {
			console.error('Bulk clone to UPS failed:', error);
			toast.add(m.bulk_clone_to_ups_failed(), m.error(), 'error');
		}
	}

	async function healSelectedPals() {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return;
		if (selectedIds.size === 0) return;

		send(MessageType.HEAL_PALS, [...selectedIds]);

		Object.values(appState.selectedPlayer.pals).forEach((pal) => {
			if (selectedIds.has(pal.instance_id)) {
				pal.hp = pal.max_hp;
				pal.sanity = 100;
				const palData = palsData.getByKey(pal.character_key);
				if (palData) {
					pal.stomach = palData.max_full_stomach;
				}
			}
		});

		selectedIds.clear();
	}

	async function maxSelectedPals() {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return;
		if (selectedIds.size === 0) return;

		for (const palId of selectedIds) {
			const pal = appState.selectedPlayer.pals[palId];
			handleMaxOutPal(pal, appState.selectedPlayer);
		}
		try {
			await appState.saveState();
		} catch (error) {
			console.error('Error saving state:', error);
		}
	}

	async function deleteSelectedPals() {
		if (selectedIds.size === 0) return;

		const count = selectedIds.size;
		const confirmed = await modal.showConfirmModal({
			title: m.delete_selected_entity({ entity: m.pal({ count }) }),
			message: m.delete_count_entities_confirm({
				count,
				entity: m.pal({ count })
			}),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});

		if (appState.selectedPlayer && appState.selectedPlayer.pals && confirmed) {
			send(MessageType.DELETE_PALS, {
				player_id: appState.selectedPlayer.uid,
				pal_ids: [...selectedIds]
			});

			appState.selectedPlayer.pals = Object.fromEntries(
				Object.entries(appState.selectedPlayer.pals).filter(([id]) => !selectedIds.has(id))
			);
		}

		selectedIds.clear();
	}

	async function handleDeletePal(pal: Pal) {
		const confirmed = await modal.showConfirmModal({
			title: m.delete_entity({ entity: c.pal }),
			message: m.delete_entity_by_name_confirm({ name: pal.nickname || pal.name }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});
		if (appState.selectedPlayer && appState.selectedPlayer.pals && confirmed) {
			send(MessageType.DELETE_PALS, {
				player_id: appState.selectedPlayer.uid,
				pal_ids: [pal.instance_id]
			});
			appState.selectedPlayer.pals = Object.fromEntries(
				Object.entries(appState.selectedPlayer.pals).filter(([id]) => id !== pal.instance_id)
			);
		}
	}

	function handleHealAll() {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return;
		send(MessageType.HEAL_ALL_PALS, {
			player_id: appState.selectedPlayer.uid
		});
		Object.values(appState.selectedPlayer.pals).forEach((pal) => {
			pal.hp = pal.max_hp;
			pal.sanity = 100;
			pal.is_sick = false;
			const palData = palsData.getByKey(pal.character_key);
			if (palData) {
				pal.stomach = palData.max_full_stomach;
			}
		});
	}

	async function handleSelectPreset() {
		const ids = [...selectedIds];
		const selectedPalsData = ids.map((id) => {
			const palWithData = boxPals.find((entry) => entry.id === id);
			return {
				character_id: palWithData?.pal.character_id,
				character_key: palWithData?.pal.character_key
			};
		});
		const otomoPalsData = ids.map((id) => {
			const palWithData = otomoContainer[id];
			return {
				character_id: palWithData?.character_id,
				character_key: palWithData?.character_key
			};
		});
		const allPals = [...selectedPalsData, ...otomoPalsData];

		// @ts-ignore
		const result = await modal.showModal<string>(PalPresetSelectModal, {
			title: m.select_entity({ entity: `${c.pal} ${m.preset({ count: 1 })}` }),
			selectedPals: allPals
		});
		if (!result) return;

		const presetProfile = presetsData.presetProfiles[result];

		ids.forEach((id) => {
			const palWithData = boxPals.find((entry) => entry.id === id);
			if (palWithData) {
				applyPalPreset(palWithData.pal, presetProfile, appState.selectedPlayer!);
			}

			const otomoPal = otomoContainer[id];
			if (otomoPal) {
				applyPalPreset(otomoPal, presetProfile, appState.selectedPlayer!);
			}
		});
	}

	async function addAllPalsToBox() {
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		await modal.showModal<string>(FillPalsModal, {
			title: m.add_all_pals_to_entity({ entity: m.palbox(), pals: c.pals }),
			player: appState.selectedPlayer,
			target: 'pal-box'
		});
	}

	function palActions(entry: PalWithData): ActionDescriptor[] {
		if (!isRealPal(entry)) {
			return [
				{
					id: 'palbox-pal-add',
					label: m.add_new_pal(p.pal),
					icon: 'tabler:plus',
					run: () => handleAddPal('palbox', entry.pal.storage_slot)
				}
			];
		}

		return [
			{
				id: 'palbox-pal-move',
				label: m.move_to_entity({ entity: m.party() }),
				icon: 'tabler:archive-off',
				run: () => handleMoveToParty(entry.pal)
			},
			{
				id: 'palbox-pal-clone',
				label: m.clone_selected_pal(p.pal),
				icon: 'tabler:copy',
				run: () => handleClonePal(entry.pal)
			},
			{
				id: 'palbox-pal-clone-to-ups',
				label: m.clone_to_entity({ entity: m.ups() }),
				icon: 'tabler:upload',
				run: () => handleCloneToUps(entry.pal)
			},
			{
				id: 'palbox-pal-delete',
				label: m.delete_entity({ entity: c.pal }),
				icon: 'tabler:trash',
				run: () => handleDeletePal(entry.pal),
				danger: true
			}
		];
	}
</script>

{#snippet party()}
	<div class="flex flex-col space-y-2">
		{#each Object.values(otomoContainer) as pal, index}
			<PalCard
				pal={otomoContainer[pal.instance_id]}
				selected={selectedIdList}
				onSelect={handlePalSelect}
				onMove={() => handleMoveToPalbox(pal)}
				onDelete={() => handleDeletePal(pal)}
				onAdd={() => handleAddPal('party', index)}
				onClone={() => handleClonePal(pal)}
				onCloneToUps={() => handleCloneToUps(pal)}
				showCloneToUps={true}
			/>
		{/each}
	</div>
{/snippet}

{#snippet stats()}
	{#if boxPals.length > 0}
		<PalContainerStats pals={boxPals} {elementTypes} />
	{:else}
		<div>{m.no_pals_available(p.pals)}</div>
	{/if}
{/snippet}

<!-- The page's own search box: selecting every match needs the query. -->
{#snippet filters()}
	<div id="palbox-filters" class="flex flex-col gap-4">
		<Input
			type="text"
			inputClass="w-full"
			placeholder={m.search_by_name_nickname()}
			bind:value={searchQuery}
		/>

		<div>
			<legend class="font-bold">{m.sort()}</legend>
			<hr />
			<div class="grid grid-cols-3 sm:grid-cols-6">
				<Tooltip label={m.sort_by_entity({ entity: m.level() })}>
					<button
						type="button"
						class={sortButtonClass('level')}
						onclick={() => toggleSort('level')}
					>
						<Icon icon={LevelSortIcon} />
					</button>
				</Tooltip>
				<Tooltip label={m.sort_by_entity({ entity: m.name() })}>
					<button type="button" class={sortButtonClass('name')} onclick={() => toggleSort('name')}>
						<Icon icon={NameSortIcon} />
					</button>
				</Tooltip>
				<Tooltip label={m.sort_by_entity({ entity: `${m.paldeck()} #` })}>
					<button
						type="button"
						class={sortButtonClass('paldeck-index')}
						onclick={() => toggleSort('paldeck-index')}
					>
						<Icon icon={PaldeckSortIcon} />
					</button>
				</Tooltip>
			</div>
		</div>

		<PalFilterButtons bind:selectedFilter />

		<div class="2xl:hidden">
			<legend class="font-bold">{m.party()}</legend>
			<hr class="mb-2" />
			{@render party()}
		</div>

		<div class="2xl:hidden">
			<legend class="font-bold">{m.stats()}</legend>
			<hr class="mb-2" />
			{@render stats()}
		</div>
	</div>
{/snippet}

{#snippet portrait(entry: PalWithData)}
	<PalBadge
		pal={entry.pal}
		selected={selectedIdList}
		onMove={() => handleMoveToParty(entry.pal)}
		onDelete={() => handleDeletePal(entry.pal)}
		onAdd={() => handleAddPal('palbox', entry.pal.storage_slot)}
		onClone={() => handleClonePal(entry.pal)}
		onCloneToUps={() => handleCloneToUps(entry.pal)}
	/>
{/snippet}

{#snippet columns(entry: PalWithData)}
	{#if isRealPal(entry)}
		<div class="flex flex-col gap-0.5">
			<span class="truncate text-sm font-bold">{nicknameOf(entry)}</span>
			<span class="text-surface-400 text-xs">
				{m.level()}
				{entry.pal.level ?? 0}
			</span>
		</div>
	{:else}
		<span class="text-surface-500 text-sm">{m.empty()} · {entry.pal.storage_slot + 1}</span>
	{/if}
{/snippet}

{#snippet detail(entry: PalWithData)}
	<dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
		<dt class="text-surface-400">{m.nickname()}</dt>
		<dd>{nicknameOf(entry)}</dd>
		<dt class="text-surface-400">{m.level()}</dt>
		<dd>{entry.pal.level ?? 0}</dd>
		<dt class="text-surface-400">{m.hp()}</dt>
		<dd>{entry.pal.hp ?? 0} / {entry.pal.max_hp ?? 0}</dd>
	</dl>
{/snippet}

{#if appState.selectedPlayer}
	<div
		class="grid h-full w-full grid-cols-1 gap-2 p-2 2xl:grid-cols-[1fr_20%]"
		{...additionalProps}
	>
		<div class="flex min-h-0 gap-2">
			<!-- Once something is selected the view's own toolbar carries these
			     rows; two action surfaces at once would be two phone buttons in
			     the same corner. -->
			{#if selectedIds.size === 0}
				<ActionGroup id="palbox-actions" actions={palboxActions} title={m.quick_actions()} />
			{/if}

			<div class="min-w-0 flex-1">
				<PalContainerView
					pals={displayPals}
					idOf={(entry) => entry.id}
					{nicknameOf}
					storageKey="palbox"
					title={m.palbox()}
					pageSize={PALS_PER_PAGE}
					actions={palboxActions}
					{selection}
					{filters}
					{portrait}
					{columns}
					{detail}
					{palActions}
					onOpenPal={handleOpenPal}
				/>
			</div>
		</div>

		<aside class="hidden min-h-0 flex-col gap-2 overflow-y-auto 2xl:flex">
			<Card rounded="rounded-sm">
				<h4 class="h4 mb-2">{m.party()}</h4>
				{@render party()}
			</Card>
			<Card class="min-h-0">
				{@render stats()}
			</Card>
		</aside>
	</div>
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">{m.select_player_view_entity({ entity: m.palbox() })}</h2>
	</div>
{/if}
