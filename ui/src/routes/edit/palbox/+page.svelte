<script lang="ts">
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { elementsData, palsData, presetsData } from '$lib/data';
	import { getAppState, getModalState, getToastState, getUpsState } from '$states';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { Input, Tooltip, TooltipButton } from '$components/ui';
	import {
		NumberInputModal,
		PalSelectModal,
		PalPresetSelectModal,
		FillPalsModal,
		CloneToUpsModal
	} from '$components/modals';
	import {
		type ElementType,
		type Pal,
		type PalData,
		MessageType,
		type CloneToUpsModalProps
	} from '$types';
	import {
		assetLoader,
		debounce,
		calculateFilters,
		deepCopy,
		handleMaxOutPal,
		formatNickname,
		applyPalPreset
	} from '$utils';
	import { cn } from '$theme';
	import { staticIcons } from '$types/icons';
	import {
		Search,
		GalleryVerticalEnd,
		ArrowDown01,
		ArrowDown10,
		ArrowDownAZ,
		ArrowDownZA,
		Plus,
		CircleFadingPlus,
		Copy,
		Ambulance,
		Trash,
		X,
		ArrowDownWideNarrow,
		ArrowDownNarrowWide,
		User,
		ReplaceAll,
		BicepsFlexed,
		Bandage,
		Play,
		Info,
		Upload
	} from 'lucide-svelte';
	import { Card } from '$components/ui';
	import { PalCard, PalBadge, PalContainerStats } from '$components';
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

	// Derived classes for filters
	const sortLuckyClass = $derived(
		cn('btn', selectedFilter === 'lucky' ? 'bg-secondary-500/25' : '')
	);
	const sortAlphaClass = $derived(
		cn('btn', selectedFilter === 'alpha' ? 'bg-secondary-500/25' : '')
	);
	const sortHumanClass = $derived(
		cn('btn', selectedFilter === 'human' ? 'bg-secondary-500/25' : '')
	);
	const sortPredatorClass = $derived(
		cn('btn', selectedFilter === 'predator' ? 'bg-secondary-500/25' : '')
	);
	const sortOilrigClass = $derived(
		cn('btn', selectedFilter === 'oilrig' ? 'bg-secondary-500/25' : '')
	);
	const sortSummonClass = $derived(
		cn('btn', selectedFilter === 'summon' ? 'bg-secondary-500/25' : '')
	);

	const sortButtonClass = (currentSortBy: SortBy) =>
		cn('btn', sortBy === currentSortBy ? 'bg-secondary-500/25' : '');

	const elementClass = (element: string) =>
		cn('btn', selectedFilter === element ? 'bg-secondary-500/25' : '');

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
	const elementIcons = $derived.by(() => {
		let elementIcons: Record<string, string> = {};
		for (const element of elementTypes) {
			const elementData = elementsData.elements[element];
			if (elementData) {
				elementIcons[element] = assetLoader.loadImage(
					`${ASSET_DATA_PATH}/img/${elementData.icon}.webp`
				) as string;
			}
		}
		return elementIcons;
	});

	const LevelSortIcon = $derived.by(() => {
		if (sortBy !== 'level') {
			return ArrowDown01;
		} else {
			return sortOrder === 'asc' ? ArrowDown01 : ArrowDown10;
		}
	});

	const NameSortIcon = $derived.by(() => {
		if (sortBy !== 'name') {
			return ArrowDownAZ;
		} else {
			return sortOrder === 'asc' ? ArrowDownAZ : ArrowDownZA;
		}
	});

	const PaldeckSortIcon = $derived.by(() => {
		if (sortBy !== 'paldeck-index') {
			return ArrowDownWideNarrow;
		} else {
			return sortOrder === 'asc' ? ArrowDownWideNarrow : ArrowDownNarrowWide;
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
			const matchesElement = elementTypes.includes(selectedFilter)
				? palData.element_types
						.map((e: ElementType) => e.toString()!.toLowerCase())
						.includes(selectedFilter.toLowerCase())
				: true;
			const matchesAlpha = selectedFilter === 'alpha' ? pal.is_boss : true;
			const matchesLucky = selectedFilter === 'lucky' ? pal.is_lucky : true;
			const matchesHuman = selectedFilter === 'human' ? !palData?.is_pal : true;
			const matchesPredator =
				selectedFilter === 'predator' ? pal.character_id.toLowerCase().includes('predator_') : true;
			const matchesOilrig =
				selectedFilter === 'oilrig' ? pal.character_id.toLowerCase().includes('_oilrig') : true;
			const matchesSummon =
				selectedFilter === 'summon' ? pal.character_id.toLowerCase().includes('summon_') : true;
			return (
				matchesSearch &&
				matchesElement &&
				matchesAlpha &&
				matchesLucky &&
				matchesHuman &&
				matchesPredator &&
				matchesOilrig &&
				matchesSummon
			);
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
			message: m.clone_selected_to_entity({ pals: c.pals, entity: c.universalPalStorage }),
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

			toast.add(
				m.successfully_cloned_pal_to_entity({
					pal: pal.nickname || pal.name,
					entity: c.universalPalStorage
				}),
				m.success(),
				'success'
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
			message: m.clone_selected_pals_to_entity({
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
				selectedPals,
				'pal_box',
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
			<nav class="btn-group bg-surface-900 mb-2 w-full items-center overflow-x-auto rounded-sm p-1">
				<Tooltip position="right" label={m.add_new_pal_to_entity({ entity: m.palbox() })}>
					<button
						class="btn hover:preset-tonal-secondary p-2"
						onclick={() => handleAddPal('palbox')}
					>
						<Plus class="h-4 w-4" />
					</button>
				</Tooltip>
				<Tooltip
					position="right"
					label={m.add_all_pals_to_entity({ entity: m.palbox(), pals: c.pals })}
				>
					<button class="btn hover:preset-tonal-secondary p-2" onclick={() => addAllPalsToBox()}>
						<CircleFadingPlus class="h-4 w-4" />
					</button>
				</Tooltip>
				<Tooltip>
					<button
						class="btn hover:preset-tonal-secondary p-2"
						onclick={(event) => handleSelectAll(event)}
					>
						<ReplaceAll class="h-4 w-4" />
					</button>
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
					<button class="btn hover:preset-tonal-secondary p-2" onclick={handleHealAll}>
						<Bandage class="h-4 w-4" />
					</button>
				</Tooltip>
				{#if selectedPals.length === 1}
					<Tooltip label={m.clone_selected_pal(p.pal)}>
						<button class="btn hover:preset-tonal-secondary p-2" onclick={cloneSelectedPal}>
							<Copy class="h-4 w-4" />
						</button>
					</Tooltip>
				{/if}
				{#if selectedPals.length >= 1}
					<Tooltip
						label={m.apply_preset_to_selected({ pals: m.pal({ count: selectedPals.length }) })}
					>
						<button class="btn hover:preset-tonal-secondary p-2" onclick={handleSelectPreset}>
							<Play class="h-4 w-4" />
						</button>
					</Tooltip>
					<Tooltip
						label={m.clone_pals_to_entity({
							count: selectedPals.length,
							pals: m.pal({ count: selectedPals.length }),
							entity: c.universalPalStorage
						})}
					>
						<button class="btn hover:preset-tonal-secondary p-2" onclick={handleBulkCloneToUps}>
							<Upload class="h-4 w-4" />
						</button>
					</Tooltip>
					<Tooltip label={m.heal_selected_pals({ pals: m.pal({ count: selectedPals.length }) })}>
						<button class="btn hover:preset-tonal-secondary p-2" onclick={healSelectedPals}>
							<Ambulance class="h-4 w-4" />
						</button>
					</Tooltip>
					<Tooltip label={m.max_out_selected_pals({ pals: m.pal({ count: selectedPals.length }) })}>
						<button class="btn hover:preset-tonal-secondary p-2" onclick={maxSelectedPals}>
							<BicepsFlexed class="h-4 w-4" />
						</button>
					</Tooltip>
					<Tooltip
						label={m.delete_selected_entity({ entity: m.pal({ count: selectedPals.length }) })}
					>
						<button class="btn hover:preset-tonal-secondary p-2" onclick={deleteSelectedPals}>
							<Trash class="h-4 w-4" />
						</button>
					</Tooltip>
					<Tooltip
						label={m.clear_selected_entity({ entity: m.pal({ count: selectedPals.length }) })}
					>
						<button
							class="btn hover:preset-tonal-secondary p-2"
							onclick={() => (selectedPals = [])}
						>
							<X class="h-4 w-4" />
						</button>
					</Tooltip>
				{/if}
			</nav>
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
					{#snippet lead()}<Search />{/snippet}
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
							<div class="grid grid-cols-6">
								<Tooltip label={m.sort_by_entity({ entity: m.level() })}>
									<button
										type="button"
										class={sortButtonClass('level')}
										onclick={() => toggleSort('level')}
									>
										<LevelSortIcon />
									</button>
								</Tooltip>
								<Tooltip label={m.sort_by_entity({ entity: m.name() })}>
									<button
										type="button"
										class={sortButtonClass('name')}
										onclick={() => toggleSort('name')}
									>
										<NameSortIcon />
									</button>
								</Tooltip>
								<Tooltip label={m.sort_by_entity({ entity: `${m.paldeck()} #` })}>
									<button
										type="button"
										class={sortButtonClass('paldeck-index')}
										onclick={() => toggleSort('paldeck-index')}
									>
										<PaldeckSortIcon />
									</button>
								</Tooltip>
							</div>
						</div>
						<div>
							<legend class="font-bold">{m.element_and_type()}</legend>
							<hr />
							<div class="mt-2 grid grid-cols-4 2xl:grid-cols-6">
								<Tooltip>
									<button class={elementClass('All')} onclick={() => (selectedFilter = 'All')}>
										<GalleryVerticalEnd />
									</button>
									{#snippet popup()}{m.all_entity({ entity: c.pals })}{/snippet}
								</Tooltip>
								{#each [...elementTypes] as element}
									{@const localizedName = elementsData.getByKey(element)?.localized_name}
									<Tooltip label={localizedName}>
										<button
											class={elementClass(element)}
											onclick={() => (selectedFilter = element)}
											aria-label={localizedName}
										>
											<img
												src={elementIcons[element]}
												alt={localizedName}
												class="pal-element-badge"
											/>
										</button>
									</Tooltip>
								{/each}
								<Tooltip label={c.alphaPals}>
									<button
										type="button"
										class={sortAlphaClass}
										onclick={() => (selectedFilter = 'alpha')}
									>
										<img src={staticIcons.alphaIcon} alt="Alpha" class="pal-element-badge" />
									</button>
								</Tooltip>
								<Tooltip label={c.luckyPals}>
									<button
										type="button"
										class={sortLuckyClass}
										onclick={() => (selectedFilter = 'lucky')}
									>
										<img src={staticIcons.luckyIcon} alt="Alpha" class="pal-element-badge" />
									</button>
								</Tooltip>
								<Tooltip label={c.humans}>
									<button
										type="button"
										class={sortHumanClass}
										onclick={() => (selectedFilter = 'human')}
									>
										<User />
									</button>
								</Tooltip>
								<Tooltip label={c.predatorPals}>
									<button
										type="button"
										class={sortPredatorClass}
										onclick={() => (selectedFilter = 'predator')}
									>
										<img
											src={staticIcons.predatorIcon}
											alt="Predator"
											class="pal-element-badge"
											style="filter: {calculateFilters('#FF0000')};"
										/>
									</button>
								</Tooltip>
								<Tooltip label={c.oilRigPals}>
									<button
										type="button"
										class={sortOilrigClass}
										onclick={() => (selectedFilter = 'oilrig')}
									>
										<img src={staticIcons.oilrigIcon} alt="Oil Rig" class="pal-element-badge" />
									</button>
								</Tooltip>
								<Tooltip label={c.summonedPals}>
									<button
										type="button"
										class={sortSummonClass}
										onclick={() => (selectedFilter = 'summon')}
									>
										<img src={staticIcons.altarIcon} alt="Summoned" class="pal-element-badge" />
									</button>
								</Tooltip>
							</div>
						</div>
					{/snippet}
				</Accordion.Item>
				<Accordion.Item
					value="stats"
					base="block 2xl:hidden rounded-sm bg-surface-900"
					controlHover="hover:bg-secondary-500/25"
				>
					{#snippet lead()}<Info />{/snippet}
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
					{#snippet lead()}<User />{/snippet}
					{#snippet control()}
						<span class="font-bold">{m.party()}</span>
					{/snippet}
					{#snippet panel()}
						{@render party()}
					{/snippet}
				</Accordion.Item>
			</Accordion>

			<Card rounded="rounded-sm" class="mt-2 hidden 2xl:block">
				<h4 class="h4 mb-2">{m.party()}</h4>
				{@render party()}
			</Card>
		</div>

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
							popupLabel={`Box ${page}`}
						>
							{Math.floor(page)}
						</TooltipButton>
					{/each}
				</div>

				<button class="rounded-sm px-4 py-2 font-bold" onclick={incrementPage}>
					<img src={staticIcons.eIcon} alt="Next" class="h-10 w-10" />
				</button>
			</div>

			<div class="overflow-hidden">
				<div class="grid grid-cols-6 place-items-center gap-4 p-4">
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
		{#if pals && pals.length > 0}
			<Card class="h-107.5 mr-2 hidden 2xl:block">
				<PalContainerStats {pals} {elementTypes} />
			</Card>
		{:else}
			<Card class="h-107.5 mr-2 hidden 2xl:block">
				<div>{m.no_pals_available(p.pals)}</div>
			</Card>
		{/if}
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
