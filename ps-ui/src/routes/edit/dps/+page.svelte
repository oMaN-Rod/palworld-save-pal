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
		PalSelectModal,
		FillPalsModal,
		NumberInputModal,
		PalPresetSelectModal,
		CloneToUpsModal
	} from '$components/modals';
	import { MessageType, type Pal, type PalData, type CloneToUpsModalProps } from '$types';
	import {
		formatNickname,
		deepCopy,
		applyPalPreset,
		selectedStorageIndexes,
		storageIndexOf,
		withoutStorageIndexes,
		palMatchesFilter
	} from '$utils';
	import { cn } from '$theme';
	import { PalBadge, PalContainerStats, PalFilterButtons } from '$components/pal';
	import { PalContainerView } from '$components/pal/container';
	import type { PalContainerSelection } from '$states/palContainer.svelte';
	import { send } from '$lib/utils/websocketUtils';
	import * as m from '$i18n/messages';
	import { c, p } from '$lib/utils/commonTranslations';

	import { buildDpsActions } from './dpsActions';

	const PALS_PER_PAGE = 30;
	const TOTAL_SLOTS = 9600;

	type SortBy = 'name' | 'level' | 'paldeck-index' | 'slot-index';
	type SortOrder = 'asc' | 'desc';

	type PalWithData = {
		id: string;
		/** The storage key of the slot, as a number: the backend rejects "3". */
		index: number;
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

	const dpsPals = $derived.by((): PalWithData[] => {
		const storage = appState.selectedPlayer?.dps;
		if (!storage) return [];
		return Object.entries(storage)
			.filter(([_, pal]) => pal && pal.character_id !== 'None')
			.map(([index, pal]) => ({
				id: pal.instance_id,
				index: Number(index),
				pal,
				palData: palsData.getByKey(pal.character_key)
			}));
	});

	const palsById = $derived(new Map(dpsPals.map((entry) => [entry.id, entry])));

	const knownPals = $derived(dpsPals.filter(({ palData }) => Boolean(palData)));

	const matchingPals = $derived.by(() => {
		const query = searchQuery.toLowerCase();
		return knownPals.filter(({ pal, palData }) => {
			const matchesSearch =
				query === '' ||
				Boolean(palData?.localized_name?.toLowerCase().includes(query)) ||
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
			if (!bySlot.has(entry.index)) {
				bySlot.set(entry.index, entry);
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

	const dpsActions = $derived(
		buildDpsActions({
			selectionCount: selectedIds.size,
			addAllPals: addAllPalsDps,
			selectAll,
			applyPreset: handleSelectPreset,
			cloneSelectedToUps: handleBulkCloneToUps,
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
			index,
			pal: {
				character_id: 'None',
				character_key: 'None',
				storage_slot: index,
				instance_id: `empty-${index}`
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
		if (!palsById.has(id)) return;
		if (selectedIds.has(id)) {
			selectedIds.delete(id);
		} else {
			selectedIds.add(id);
		}
	}

	function selectAll(): void {
		const ids = sortedPals.map((entry) => entry.id);
		const wasComplete = selectedIds.size === ids.length;

		selectedIds.clear();
		if (wasComplete) return;
		for (const id of ids) selectedIds.add(id);
	}

	function handleOpenPal(entry: PalWithData): void {
		if (!isRealPal(entry)) {
			handleAddPal(entry.index);
			return;
		}
		palEditor.open(entry.pal);
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

	async function handleAddPal(index: number) {
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		const result = await modal.showModal<[string, string] | undefined>(PalSelectModal, {
			title: m.add_new_pal_to_entity({ entity: c.dimensionalPalStorage })
		});
		if (!result) return;
		const [selectedPal, nickname] = result;
		const palData = palsData.getByKey(selectedPal);

		send(MessageType.ADD_DPS_PAL, {
			player_id: appState.selectedPlayer.uid,
			character_id: selectedPal,
			nickname:
				nickname ||
				formatNickname(palData?.localized_name || selectedPal, appState.settings.new_pal_prefix),
			storage_slot: index
		});
	}

	async function clonePal(pal: Pal) {
		const maxClones = appState.selectedPlayer!.dps
			? TOTAL_SLOTS - Object.values(appState.selectedPlayer!.dps).length
			: 0;
		if (maxClones === 0) {
			toast.add(
				m.no_slots_available_in_entity({ entity: c.dimensionalPalStorage }),
				m.error(),
				'error'
			);
			return;
		}
		// @ts-ignore
		const result = await modal.showModal<number>(NumberInputModal, {
			title: m.how_many_clones(),
			message: m.slots_available_in_entity({ count: maxClones, entity: c.dimensionalPalStorage }),
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
			send(MessageType.CLONE_DPS_PAL, {
				pal: clonedPal
			});
		}
	}

	async function handleClonePal(pal: Pal) {
		await clonePal(pal);
	}

	async function handleCloneToUps(pal: Pal) {
		// @ts-ignore
		const result = await modal.showModal<CloneToUpsModalProps>(CloneToUpsModal, {
			title: m.clone_to_entity({ entity: c.universalPalStorage }),
			message: m.clone_pal_to_entity({
				entity: c.universalPalStorage,
				pal: pal.nickname || pal.name || pal.character_id
			}),
			pals: [pal]
		});

		if (!result) return;

		const { collectionId, tags, notes } = result;

		try {
			await upsState.cloneToUps(
				[pal.instance_id],
				'dps',
				appState.selectedPlayer?.uid,
				collectionId,
				tags.length > 0 ? tags : undefined,
				notes || undefined
			);

			toast.add(
				m.successfully_cloned_pal_to_entity({
					pal: pal.nickname || pal.name || pal.character_id,
					entity: c.universalPalStorage
				}),
				m.success(),
				'success'
			);
		} catch (error) {
			console.error('Clone to UPS failed:', error);
			toast.add(m.clone_to_entity_failed({ entity: m.ups() }), m.error(), 'error');
		}
	}

	async function handleBulkCloneToUps() {
		if (selectedIds.size === 0) return;

		const ids = selectedIdList;
		const palsToClone = ids.map((id) => palsById.get(id)?.pal).filter(Boolean) as Pal[];

		if (palsToClone.length === 0) return;

		// @ts-ignore
		const result = await modal.showModal<CloneToUpsModalProps>(CloneToUpsModal, {
			title: m.clone_to_entity({ entity: c.universalPalStorage }),
			message: m.clone_pals_to_entity({
				count: palsToClone.length,
				pals: m.pal({ count: palsToClone.length }),
				entity: c.universalPalStorage
			}),
			pals: palsToClone
		});

		if (!result) return;

		const { collectionId, tags, notes } = result;

		try {
			await upsState.cloneToUps(
				ids,
				'dps',
				appState.selectedPlayer?.uid,
				collectionId,
				tags.length > 0 ? tags : undefined,
				notes || undefined
			);

			toast.add(
				m.successfully_cloned_pals_to_entity({
					count: palsToClone.length,
					pals: m.pal({ count: palsToClone.length }),
					entity: c.universalPalStorage
				}),
				m.success(),
				'success'
			);

			selectedIds.clear();
		} catch (error) {
			console.error('Bulk clone to UPS failed:', error);
			toast.add(m.bulk_clone_to_ups_failed(), m.error(), 'error');
		}
	}

	async function handleSelectPreset() {
		const ids = selectedIdList;
		const selectedPalsData = ids.map((id) => {
			const palWithData = palsById.get(id);
			return {
				character_id: palWithData?.pal.character_id,
				character_key: palWithData?.pal.character_key
			};
		});

		// @ts-ignore
		const result = await modal.showModal<string>(PalPresetSelectModal, {
			title: m.select_entity({ entity: c.preset }),
			selectedPals: selectedPalsData
		});
		if (!result) return;

		const presetProfile = presetsData.presetProfiles[result];

		ids.forEach((id) => {
			const palWithData = palsById.get(id);
			if (palWithData) {
				applyPalPreset(palWithData.pal, presetProfile, appState.selectedPlayer!);
			}
		});
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

		if (appState.selectedPlayer && appState.selectedPlayer.dps && confirmed) {
			const palIndexes = selectedStorageIndexes(appState.selectedPlayer.dps, selectedIdList);
			send(MessageType.DELETE_DPS_PALS, {
				player_id: appState.selectedPlayer.uid,
				pal_indexes: palIndexes
			});

			appState.selectedPlayer.dps = withoutStorageIndexes(appState.selectedPlayer.dps, palIndexes);
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
		if (appState.selectedPlayer && appState.selectedPlayer.dps && confirmed) {
			const palIndex = storageIndexOf(appState.selectedPlayer.dps, pal.instance_id);
			if (palIndex === undefined) return;
			send(MessageType.DELETE_DPS_PALS, {
				player_id: appState.selectedPlayer.uid,
				pal_indexes: [palIndex]
			});
			appState.selectedPlayer.dps = withoutStorageIndexes(appState.selectedPlayer.dps, [palIndex]);
		}
	}

	async function addAllPalsDps() {
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		await modal.showModal<string>(FillPalsModal, {
			title: m.fill_entity({ entity: c.dimensionalPalStorage }),
			player: appState.selectedPlayer,
			target: 'dps'
		});
	}

	function palActions(entry: PalWithData): ActionDescriptor[] {
		if (!isRealPal(entry)) {
			return [
				{
					id: 'dps-pal-add',
					label: m.add_new_pal(p.pal),
					icon: 'tabler:plus',
					run: () => handleAddPal(entry.index)
				}
			];
		}

		return [
			{
				id: 'dps-pal-clone',
				label: m.clone_selected_pal(p.pal),
				icon: 'tabler:copy',
				run: () => handleClonePal(entry.pal)
			},
			{
				id: 'dps-pal-clone-to-ups',
				label: m.clone_to_entity({ entity: m.ups() }),
				icon: 'tabler:upload',
				run: () => handleCloneToUps(entry.pal)
			},
			{
				id: 'dps-pal-delete',
				label: m.delete_entity({ entity: c.pal }),
				icon: 'tabler:trash',
				run: () => handleDeletePal(entry.pal),
				danger: true
			}
		];
	}
</script>

{#snippet stats()}
	{#if dpsPals.length > 0}
		<PalContainerStats pals={dpsPals} {elementTypes} />
	{:else}
		<div>{m.no_pals_available(p.pals)}</div>
	{/if}
{/snippet}

<!-- The page's own search box: its filtering also sorts and pads, which `matches` can't express. -->
{#snippet filters()}
	<div id="dps-filters" class="flex flex-col gap-4">
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
				<Tooltip label={m.sort_by_entity({ entity: m.paldeck() })}>
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
		onDelete={() => handleDeletePal(entry.pal)}
		onAdd={() => handleAddPal(entry.index)}
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
		<span class="text-surface-500 text-sm">{m.empty()} · {entry.index + 1}</span>
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
			<!-- The view's own toolbar carries these rows once something is selected. -->
			{#if selectedIds.size === 0}
				<ActionGroup id="dps-actions" actions={dpsActions} title={m.quick_actions()} />
			{/if}

			<div class="min-w-0 flex-1">
				<PalContainerView
					pals={displayPals}
					idOf={(entry) => entry.id}
					{nicknameOf}
					storageKey="dps"
					title={c.dimensionalPalStorage}
					pageSize={PALS_PER_PAGE}
					actions={dpsActions}
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

		<aside id="dps-stats" class="hidden min-h-0 flex-col gap-2 overflow-y-auto 2xl:flex">
			<Card class="min-h-0">
				{@render stats()}
			</Card>
		</aside>
	</div>
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">{m.select_player_view_entity({ entity: c.dimensionalPalStorage })}</h2>
	</div>
{/if}
