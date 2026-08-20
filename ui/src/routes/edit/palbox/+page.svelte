<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { elementsData, palsData, presetsData } from '$lib/data';
	import { getAppState, getModalState, getToastState, getUpsState } from '$states';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { Button, Input, Tooltip, TooltipButton } from '$components/ui';
	import {
		NumberInputModal,
		PalSelectModal,
		PalPresetSelectModal,
		FillPalsModal,
		CloneToUpsModal
	} from '$components/modals';
	import { type Pal, type PalData, MessageType, type CloneToUpsModalProps } from '$types';
	import {
		debounce,
		deepCopy,
		handleMaxOutPal,
		formatNickname,
		applyPalPreset,
		palMatchesFilter
	} from '$utils';
	import { cn } from '$theme';
	import { staticIcons } from '$types/icons';
	import { Card } from '$components/ui';
	import { PalCard, PalBadge, PalContainerStats, PalFilterButtons } from '$components/pal';
	import { send } from '$lib/utils/websocketUtils';
	import type { ValueChangeDetails } from '@zag-js/accordion';
	import * as m from '$i18n/messages';
	import { c, p } from '$lib/utils/commonTranslations';

	const PALS_PER_PAGE = 30;
	const TOTAL_SLOTS = 960;
	const VISIBLE_PAGE_BUBBLES = 16;

	type SortBy = 'name' | 'level' | 'paldeck-index' | 'slot-index';
	type SortOrder = 'asc' | 'desc';

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();
	const upsState = getUpsState();

	let { ...additionalProps } = $props<{
		[key: string]: any;
	}>();

	let searchQuery = $state('');
	let selectedFilter = $state('All');
	let currentPage = $state(1);
	let filteredPals: PalWithData[] = $state([]);
	let selectedPals: string[] = $state([]);
	let sortBy: SortBy = $state('slot-index');
	let sortOrder: SortOrder = $state('asc');
	let filterExpand = $state(['']);

	type PalWithData = {
		id: string;
		pal: Pal;
		palData?: PalData;
	};

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

			// Convert the array back to an object
			return Object.fromEntries(allSlots);
		}
	});

	const totalPages = $derived(
		Math.ceil(
			searchQuery || selectedFilter !== 'All' || sortBy !== 'slot-index'
				? filteredPals.length
				: TOTAL_SLOTS
		) / PALS_PER_PAGE
	);
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

	const currentPageItems = $derived.by(() => {
		const startIndex = (currentPage - 1) * PALS_PER_PAGE;
		const endIndex = startIndex + PALS_PER_PAGE;

		if (searchQuery || selectedFilter !== 'All' || sortBy !== 'slot-index') {
			return filteredPals.slice(startIndex, endIndex);
		}

		const paddedPals = Array(TOTAL_SLOTS)
			.fill(undefined)
			.map((_, index) => {
				const pal = filteredPals.find((p) => p.pal.storage_slot === index);
				if (pal) {
					return pal;
				} else {
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
			});

		return paddedPals.slice(startIndex, endIndex);
	});

	const sortButtonClass = (currentSortBy: SortBy) =>
		cn('btn', sortBy === currentSortBy ? 'bg-secondary-500/25' : '');

	const pals = $derived.by(() => {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return;
		const playerPals = Object.entries(appState.selectedPlayer.pals as Record<string, Pal>);
		const palBoxId = appState.selectedPlayer.pal_box_id;
		return playerPals
			.filter(([_, pal]) => pal.storage_id === palBoxId)
			.map(([id, pal]) => {
				const palData = palsData.getByKey(pal.character_key);
				return { id, pal, palData } as PalWithData;
			});
	});

	const elementTypes = $derived(Object.keys(elementsData.elements));

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
			return sortOrder === 'asc' ? 'tabler:arrows-sort' : 'tabler:arrows-sort';
		}
	});

	function handleKeydown(event: KeyboardEvent) {
		if (event.target instanceof HTMLInputElement) {
			return;
		}
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
	}

	function incrementPage() {
		if (currentPage < totalPages) {
			currentPage++;
		} else {
			currentPage = 1;
		}
	}

	const debouncedFilterPals = debounce(filterPals, 300);

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

	async function filterPals() {
		if (!pals) return;
		filteredPals = pals.filter(({ pal, palData }) => {
			if (!palData) {
				return false;
			}
			const matchesSearch =
				pal.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
				pal.nickname?.toLowerCase().includes(searchQuery.toLowerCase()) ||
				pal.character_id.toLowerCase().includes(searchQuery.toLowerCase());
			return matchesSearch && palMatchesFilter(pal, palData, selectedFilter);
		});

		sortPals();
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
		sortPals();
	}

	function sortPals() {
		switch (sortBy) {
			case 'name':
				sortByName();
				break;
			case 'level':
				sortByLevel();
				break;
			case 'paldeck-index':
				sortByPaldeckIndex();
				break;
			default:
				sortBySlotIndex();
				break;
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

	function sortByName() {
		filteredPals = filteredPals.sort((a, b) =>
			sortOrder === 'asc'
				? a.pal.name.localeCompare(b.pal.name)
				: b.pal.name.localeCompare(a.pal.name)
		);
	}

	function sortByLevel() {
		filteredPals = filteredPals.sort((a, b) =>
			sortOrder === 'asc' ? a.pal.level - b.pal.level : b.pal.level - a.pal.level
		);
	}

	function sortBySlotIndex() {
		filteredPals = filteredPals.sort((a, b) =>
			sortOrder === 'asc'
				? a.pal.storage_slot - b.pal.storage_slot
				: b.pal.storage_slot - a.pal.storage_slot
		);
	}

	async function sortByPaldeckIndex() {
		const palInfos = filteredPals.map((p) => palsData.getByKey(p.pal.character_key));
		const palsWithInfo = filteredPals.map((pal, index) => [pal, palInfos[index]]);

		palsWithInfo.sort((a, b) => {
			const indexA = (a[1] as PalData)?.pal_deck_index ?? Infinity;
			const indexB = (b[1] as PalData)?.pal_deck_index ?? Infinity;
			return sortOrder === 'asc' ? indexA - indexB : indexB - indexA;
		});

		filteredPals = palsWithInfo.map((pair) => pair[0] as PalWithData);
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
			const pal = appState.selectedPlayer.pals[selectedPals[0]];
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
		if (selectedPals.length === 0) return;

		const palsToClone = selectedPals
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
				selectedPals,
				'pal_box',
				appState.selectedPlayer?.uid,
				collectionId,
				tags.length > 0 ? tags : undefined,
				notes || undefined
			);

			selectedPals = [];
		} catch (error) {
			console.error('Bulk clone to UPS failed:', error);
			toast.add(m.bulk_clone_to_ups_failed(), m.error(), 'error');
		}
	}

	function handlePalSelect(pal: Pal, event: MouseEvent) {
		if (!pal || pal.character_id === 'None') return;
		if (event.ctrlKey || event.metaKey) {
			// Toggle selection
			if (selectedPals.includes(pal.instance_id)) {
				selectedPals = selectedPals.filter((id) => id !== pal.instance_id);
			} else {
				selectedPals = [...selectedPals, pal.instance_id];
			}
		}
	}

	async function healSelectedPals() {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return;
		if (selectedPals.length === 0) return;

		send(MessageType.HEAL_PALS, [...selectedPals]);

		Object.values(appState.selectedPlayer.pals).forEach(async (pal) => {
			if (selectedPals.includes(pal.instance_id)) {
				pal.hp = pal.max_hp;
				pal.sanity = 100;
				const palData = palsData.getByKey(pal.character_key);
				if (palData) {
					pal.stomach = palData.max_full_stomach;
				}
			}
		});

		selectedPals = [];
	}

	async function maxSelectedPals() {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return;
		if (selectedPals.length === 0) return;

		for (const palId of selectedPals) {
			const pal = appState.selectedPlayer.pals[palId];
			handleMaxOutPal(pal, appState.selectedPlayer);
		}
		await appState.saveState();
	}

	async function deleteSelectedPals() {
		if (selectedPals.length === 0) return;

		const confirmed = await modal.showConfirmModal({
			title: m.delete_selected_entity({ entity: m.pal({ count: selectedPals.length }) }),
			message: m.delete_count_entities_confirm({
				count: selectedPals.length,
				entity: m.pal({ count: selectedPals.length })
			}),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});

		if (appState.selectedPlayer && appState.selectedPlayer.pals && confirmed) {
			send(MessageType.DELETE_PALS, {
				player_id: appState.selectedPlayer.uid,
				pal_ids: [...selectedPals]
			});

			appState.selectedPlayer.pals = Object.fromEntries(
				Object.entries(appState.selectedPlayer.pals).filter(([id]) => !selectedPals.includes(id))
			);
		}

		selectedPals = [];
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

	function handleSelectAll(event: MouseEvent & { currentTarget: EventTarget & HTMLButtonElement }) {
		if (event.ctrlKey || event.metaKey) {
			const otomoPalIds = Object.values(otomoContainer)
				.filter((pal) => pal.character_id !== 'None')
				.map((pal) => pal.instance_id);

			if (selectedPals.length === filteredPals.length + otomoPalIds.length) {
				selectedPals = [];
			} else {
				selectedPals = [...filteredPals.map((p) => p.id), ...otomoPalIds];
			}
		} else {
			if (selectedPals.length === filteredPals.length) {
				selectedPals = [];
			} else {
				selectedPals = filteredPals.map((p) => p.id);
			}
		}
	}

	$effect(() => {
		if (appState.selectedPlayer && appState.selectedPlayer.pals) {
			debouncedFilterPals();
		}
	});

	$effect(() => {
		if (searchQuery || selectedFilter) {
			debouncedFilterPals();
		}
	});

	$effect(() => {
		if (
			(appState.selectedPal && appState.selectedPal.level) ||
			(appState.selectedPal && appState.selectedPal.nickname)
		) {
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
		if (currentPage > totalPages) {
			currentPage = 1;
		}
	});

	$effect(() => {
		if (pals) {
			debouncedFilterPals();
		}
	});

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
		const selectedPalsData = selectedPals.map((id) => {
			const palWithData = pals?.find((p) => p.id === id);
			return {
				character_id: palWithData?.pal.character_id,
				character_key: palWithData?.pal.character_key
			};
		});
		const otomoPalsData = selectedPals.map((id) => {
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

		selectedPals.forEach((id) => {
			const palWithData = pals?.find((p) => p.id === id);
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
</script>

{#snippet party()}
	<div class="flex flex-col space-y-2">
		{#each Object.values(otomoContainer) as pal, index}
			<PalCard
				pal={otomoContainer[pal.instance_id]}
				bind:selected={selectedPals}
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

{#if appState.selectedPlayer}
	<div
		class="grid h-full w-full grid-cols-[25%_1fr] 2xl:grid-cols-[25%_1fr_20%]"
		{...additionalProps}
	>
		<div class="shrink-0 p-4">
			<nav
				id="palbox-toolbar"
				class="btn-group bg-surface-900 mb-2 w-full items-center overflow-x-auto rounded-sm p-1"
			>
				<Tooltip position="right" label={m.add_new_pal_to_entity({ entity: m.palbox() })}>
					<Button
						id="palbox-add-pal"
						variant="ghost"
						size="icon"
						onclick={() => handleAddPal('palbox')}
					>
						<Icon icon="tabler:plus" class="h-4 w-4" />
					</Button>
				</Tooltip>
				<Tooltip
					position="right"
					label={m.add_all_pals_to_entity({ entity: m.palbox(), pals: c.pals })}
				>
					<Button id="palbox-add-all" variant="ghost" size="icon" onclick={() => addAllPalsToBox()}>
						<Icon icon="tabler:circle-plus" class="h-4 w-4" />
					</Button>
				</Tooltip>
				<Tooltip>
					<Button
						id="palbox-select-all"
						variant="ghost"
						size="icon"
						onclick={(event: MouseEvent) =>
							handleSelectAll(
								event as MouseEvent & { currentTarget: EventTarget & HTMLButtonElement }
							)}
					>
						<Icon icon="tabler:arrows-diff" class="h-4 w-4" />
					</Button>
					{#snippet popup()}
						<div class="flex flex-col">
							<span>{m.select_all_in()}</span>
							<div class="grid grid-cols-[auto_1fr] gap-1">
								<img src={staticIcons.leftClickIcon} alt="Left Click" class="h-6 w-6" />
								<span class="text-sm">{m.palbox()}</span>
								<div class="flex">
									<img src={staticIcons.ctrlIcon} alt="Ctrl" class="h-6 w-6" />
									<img src={staticIcons.leftClickIcon} alt="Left Click" class="h-6 w-6" />
								</div>
								<span class="text-sm">{m.pal_box_party()}</span>
							</div>
						</div>
					{/snippet}
				</Tooltip>
				<Tooltip label={m.heal_all_in_entity({ entity: m.palbox() })}>
					<Button id="palbox-heal-all" variant="ghost" size="icon" onclick={handleHealAll}>
						<Icon icon="tabler:bandage" class="h-4 w-4" />
					</Button>
				</Tooltip>
				{#if selectedPals.length === 1}
					<Tooltip label={m.clone_selected_pal(p.pal)}>
						<Button variant="ghost" size="icon" onclick={cloneSelectedPal}>
							<Icon icon="tabler:copy" class="h-4 w-4" />
						</Button>
					</Tooltip>
				{/if}
				{#if selectedPals.length >= 1}
					<Tooltip
						label={m.apply_preset_to_selected({ pals: m.pal({ count: selectedPals.length }) })}
					>
						<Button variant="ghost" size="icon" onclick={handleSelectPreset}>
							<Icon icon="tabler:player-play" class="h-4 w-4" />
						</Button>
					</Tooltip>
					<Tooltip
						label={m.clone_pals_to_entity({
							count: selectedPals.length,
							pals: m.pal({ count: selectedPals.length }),
							entity: c.universalPalStorage
						})}
					>
						<Button variant="ghost" size="icon" onclick={handleBulkCloneToUps}>
							<Icon icon="tabler:upload" class="h-4 w-4" />
						</Button>
					</Tooltip>
					<Tooltip label={m.heal_selected_pals({ pals: m.pal({ count: selectedPals.length }) })}>
						<Button variant="ghost" size="icon" onclick={healSelectedPals}>
							<Icon icon="tabler:ambulance" class="h-4 w-4" />
						</Button>
					</Tooltip>
					<Tooltip label={m.max_out_selected_pals({ pals: m.pal({ count: selectedPals.length }) })}>
						<Button variant="ghost" size="icon" onclick={maxSelectedPals}>
							<Icon icon="ph:hand-fist" class="h-4 w-4" />
						</Button>
					</Tooltip>
					<Tooltip
						label={m.delete_selected_entity({ entity: m.pal({ count: selectedPals.length }) })}
					>
						<Button variant="ghost" size="icon" onclick={deleteSelectedPals}>
							<Icon icon="tabler:trash" class="h-4 w-4" />
						</Button>
					</Tooltip>
					<Tooltip
						label={m.clear_selected_entity({ entity: m.pal({ count: selectedPals.length }) })}
					>
						<Button variant="ghost" size="icon" onclick={() => (selectedPals = [])}>
							<Icon icon="tabler:x" class="h-4 w-4" />
						</Button>
					</Tooltip>
				{/if}
			</nav>
			<div id="palbox-filters">
				<Accordion
					value={filterExpand}
					onValueChange={(e: ValueChangeDetails) => (filterExpand = e.value)}
					collapsible
				>
					<Accordion.Item
						value="filter"
						base="rounded-sm bg-surface-900"
						controlHover="hover:bg-secondary-500/25"
					>
						{#snippet lead()}<Icon icon="tabler:search" />{/snippet}
						{#snippet control()}
							<span class="font-bold">{m.filter_and_sort()}</span>
						{/snippet}
						{#snippet panel()}
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
										<button
											type="button"
											class={sortButtonClass('name')}
											onclick={() => toggleSort('name')}
										>
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
						{/snippet}
					</Accordion.Item>
					<Accordion.Item
						value="stats"
						base="block 2xl:hidden rounded-sm bg-surface-900"
						controlHover="hover:bg-secondary-500/25"
					>
						{#snippet lead()}<Icon icon="tabler:info-circle" />{/snippet}
						{#snippet control()}
							<span class="font-bold">{m.stats()}</span>
						{/snippet}
						{#snippet panel()}
							{#if pals && pals.length > 0}
								<PalContainerStats {pals} {elementTypes} />
							{:else}
								<div>{m.no_pals_available(p.pals)}</div>
							{/if}
						{/snippet}
					</Accordion.Item>
					<Accordion.Item
						value="party"
						base="block 2xl:hidden rounded-sm bg-surface-900"
						controlHover="hover:bg-secondary-500/25"
					>
						{#snippet lead()}<Icon icon="tabler:user" />{/snippet}
						{#snippet control()}
							<span class="font-bold">{m.party()}</span>
						{/snippet}
						{#snippet panel()}
							{@render party()}
						{/snippet}
					</Accordion.Item>
				</Accordion>
			</div>

			<div id="palbox-party">
				<Card rounded="rounded-sm" class="mt-2 hidden 2xl:block">
					<h4 class="h4 mb-2">{m.party()}</h4>
					{@render party()}
				</Card>
			</div>
		</div>

		<div>
			<!-- Pager -->
			<div id="palbox-pager" class="mb-4 flex items-center justify-center space-x-4">
				<Button
					class="rounded-full p-0! font-bold"
					variant="ghost"
					size="md"
					onclick={decrementPage}
				>
					<img src={staticIcons.qIcon} alt="Previous" class="h-10 w-10" />
				</Button>

				<div class="flex space-x-2">
					{#each visiblePages as page}
						<TooltipButton
							buttonClass="h-8 w-8 rounded-full {page === currentPage
								? 'bg-primary-500! text-white'
								: 'bg-surface-800 hover:bg-surface-600'}"
							onclick={() => (currentPage = page)}
							popupLabel={`Box ${page}`}
							variant="ghost"
							size="md"
						>
							{Math.floor(page)}
						</TooltipButton>
					{/each}
				</div>

				<Button class="rounded-sm p-0! font-bold" variant="ghost" size="md" onclick={incrementPage}>
					<img src={staticIcons.eIcon} alt="Next" class="h-10 w-10" />
				</Button>
			</div>

			<div id="palbox-grid" class="overflow-hidden">
				<div
					class="grid grid-cols-3 place-items-center gap-4 p-4 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6"
				>
					{#each currentPageItems as item (item.pal.instance_id)}
						{#if item.pal.character_id !== 'None' || (!searchQuery && selectedFilter === 'All' && sortBy === 'slot-index')}
							<PalBadge
								pal={item.pal}
								bind:selected={selectedPals}
								onSelect={handlePalSelect}
								onMove={() => handleMoveToParty(item.pal)}
								onDelete={() => handleDeletePal(item.pal)}
								onAdd={() => handleAddPal('palbox', item.pal.storage_slot)}
								onClone={() => handleClonePal(item.pal)}
								onCloneToUps={() => handleCloneToUps(item.pal)}
							/>
						{/if}
					{/each}
				</div>
			</div>
		</div>
		<div id="palbox-stats">
			{#if pals && pals.length > 0}
				<Card class="mr-2 hidden min-h-0 2xl:block">
					<PalContainerStats {pals} {elementTypes} />
				</Card>
			{:else}
				<Card class="mr-2 hidden min-h-0 2xl:block">
					<div>{m.no_pals_available(p.pals)}</div>
				</Card>
			{/if}
		</div>
	</div>
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">{m.select_player_view_entity({ entity: m.palbox() })}</h2>
	</div>
{/if}

<style>
	.pal-element-badge {
		width: 24px;
		height: 24px;
	}
</style>
