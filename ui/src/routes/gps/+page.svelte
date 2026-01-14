<script lang="ts">
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { elementsData, palsData, presetsData } from '$lib/data';
	import { getAppState, getModalState, getToastState, getUpsState } from '$states';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { Card, Input, Tooltip, TooltipButton } from '$components/ui';
	import { Spinner } from '$components';
	import {
		PalSelectModal,
		FillPalsModal,
		NumberInputModal,
		PalPresetSelectModal,
		CloneToUpsModal
	} from '$components/modals';
	import {
		type ElementType,
		type Pal,
		type PalData,
		type CloneToUpsModalProps,
		MessageType
	} from '$types';
	import {
		assetLoader,
		debounce,
		calculateFilters,
		formatNickname,
		deepCopy,
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
		Trash,
		X,
		ArrowDownWideNarrow,
		ArrowDownNarrowWide,
		User,
		ReplaceAll,
		CircleFadingPlus,
		Info,
		Play,
		Upload
	} from 'lucide-svelte';
	import { PalBadge, PalContainerStats } from '$components';
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
		index: number;
		pal: Pal;
		palData?: PalData;
	};

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
				const pal = filteredPals.find((p) => p.index == index);
				if (pal) {
					return pal;
				} else {
					return {
						id: `empty-${index}`,
						index: index,
						pal: {
							character_id: 'None',
							character_key: 'None',
							storage_slot: index,
							instance_id: `empty-${index}`,
							storage_id: '00000000-0000-0000-0000-000000000000'
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

	let pals = $derived.by(() => {
		if (!appState.gps) return;
		const gpsPals = Object.entries(appState.gps).filter(
			([_, pal]) => pal && pal.character_id !== 'None'
		);
		return gpsPals.map(([i, pal]) => {
			const palData = palsData.getByKey(pal.character_key);
			return { id: pal.instance_id, index: i as unknown as number, pal, palData };
		});
	});

	let elementTypes = $derived(Object.keys(elementsData.elements));
	let elementIcons = $derived.by(() => {
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

	let LevelSortIcon = $derived.by(() => {
		if (sortBy !== 'level') {
			return ArrowDown01;
		} else {
			return sortOrder === 'asc' ? ArrowDown01 : ArrowDown10;
		}
	});

	let NameSortIcon = $derived.by(() => {
		if (sortBy !== 'name') {
			return ArrowDownAZ;
		} else {
			return sortOrder === 'asc' ? ArrowDownAZ : ArrowDownZA;
		}
	});

	let PaldeckSortIcon = $derived.by(() => {
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

	async function filterPals() {
		if (!pals) return;
		filteredPals = pals.filter(({ pal, palData }) => {
			if (!palData) {
				return false;
			}
			const matchesSearch =
				palData.localized_name.toLowerCase().includes(searchQuery.toLowerCase()) ||
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

	async function handleSelectPreset() {
		const selectedPalsData = selectedPals.map((id) => {
			const palWithData = pals?.find((p) => p.id === id);
			return {
				character_id: palWithData?.pal.character_id,
				character_key: palWithData?.pal.character_key
			};
		});

		// @ts-ignore
		const result = await modal.showModal<string>(PalPresetSelectModal, {
			title: m.select_entity({ entity: m.preset({ count: 1 }) }),
			selectedPals: selectedPalsData
		});
		if (!result) return;

		const presetProfile = presetsData.presetProfiles[result];

		selectedPals.forEach((id) => {
			const palWithData = pals?.find((p) => p.id === id);
			if (palWithData) {
				applyPalPreset(palWithData.pal, presetProfile, undefined);
			}
		});
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

	async function handleAddPal(index: number) {
		if (!appState.gps) return;
		// @ts-ignore
		const result = await modal.showModal<[string, string] | undefined>(PalSelectModal, {
			title: m.add_new_pal_to_entity({ entity: m.global_pal_storage({ pal: m.pal({ count: 1 }) }) })
		});
		if (!result) return;
		const [selectedPal, nickname] = result;
		const palData = palsData.getByKey(selectedPal);

		send(MessageType.ADD_GPS_PAL, {
			character_id: selectedPal,
			nickname:
				nickname ||
				formatNickname(palData?.localized_name || selectedPal, appState.settings.new_pal_prefix),
			storage_slot: index
		});
	}

	async function clonePal(pal: Pal) {
		const maxClones = appState.gps ? TOTAL_SLOTS - Object.values(appState.gps).length : 0;
		if (maxClones === 0) {
			toast.add(m.no_slots_available_in_entity({ entity: m.gps() }), m.error(), 'error');
			return;
		}
		// @ts-ignore
		const result = await modal.showModal<number>(NumberInputModal, {
			title: m.how_many_clones(),
			message: m.slots_available_in_entity({
				entity: m.global_pal_storage({ pal: m.pal({ count: 1 }) }),
				count: maxClones
			}),
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
		const palInfos = filteredPals.map((p) => palsData.pals[p.pal.character_key]);
		const palsWithInfo = filteredPals.map((pal, index) => [pal, palInfos[index]]);

		palsWithInfo.sort((a, b) => {
			const indexA = (a[1] as PalData)?.pal_deck_index ?? Infinity;
			const indexB = (b[1] as PalData)?.pal_deck_index ?? Infinity;
			return sortOrder === 'asc' ? indexA - indexB : indexB - indexA;
		});

		filteredPals = palsWithInfo.map((pair) => pair[0] as PalWithData);
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

	async function deleteSelectedPals() {
		if (selectedPals.length === 0) return;

		const confirmed = await modal.showConfirmModal({
			title: m.delete_entity({ entity: m.pal({ count: selectedPals.length }) }),
			message: m.delete_count_entities_confirm({
				entity: m.pal({ count: selectedPals.length }),
				count: selectedPals.length
			}),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});

		if (appState.gps && confirmed) {
			const palIndexes = Object.entries(appState.gps)
				.filter(([_, pal]) => selectedPals.includes(pal.instance_id))
				.map(([index]) => index);
			send(MessageType.DELETE_GPS_PALS, {
				pal_indexes: palIndexes
			});

			appState.gps = Object.fromEntries(
				Object.entries(appState.gps).filter(([idx, _]) => !palIndexes.includes(idx))
			);
		}

		selectedPals = [];
	}

	async function handleDeletePal(pal: Pal) {
		const confirmed = await modal.showConfirmModal({
			title: m.delete_entity({ entity: m.pal({ count: 1 }) }),
			message: m.delete_entity_by_name_confirm({ name: pal.nickname || pal.name }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});
		if (appState.gps && confirmed) {
			const palIndex = Object.entries(appState.gps).find(
				([_, p]) => p.instance_id === pal.instance_id
			);
			send(MessageType.DELETE_GPS_PALS, {
				pal_indexes: [palIndex![0]]
			});
			appState.gps = Object.fromEntries(
				Object.entries(appState.gps).filter(([_, p]) => p.instance_id !== pal.instance_id)
			);
		}
	}

	function handleSelectAll() {
		if (selectedPals.length === filteredPals.length) {
			selectedPals = [];
		} else {
			selectedPals = filteredPals.map((p) => p.id);
		}
	}

	$effect(() => {
		if (appState.gps) {
			debouncedFilterPals();
		}
	});

	$effect(() => {
		if (searchQuery || selectedFilter) {
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

	async function handleCloneToUps(pal: Pal) {
		// @ts-ignore
		const result = await modal.showModal<CloneToUpsModalProps>(CloneToUpsModal, {
			title: m.clone_to_entity({ entity: m.ups() }),
			message: m.clone_selected_to_entity({
				pals: m.pal({ count: 1 }),
				entity: m.universal_pal_storage({ pal: m.pal({ count: 1 }) })
			}),
			pals: [pal]
		});

		if (!result) return;

		const { collectionId, tags, notes } = result;

		try {
			await upsState.cloneToUps(
				[pal.instance_id],
				'gps',
				undefined,
				collectionId,
				tags.length > 0 ? tags : undefined,
				notes || undefined
			);

			toast.add(
				m.successfully_cloned_pals_to_entity({
					pals: c.pals,
					entity: c.universalPalStorage,
					count: 1
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
		if (selectedPals.length === 0) return;

		const palsToClone = selectedPals
			.map((id) => pals?.find((p) => p.id === id)?.pal)
			.filter(Boolean) as Pal[];

		if (palsToClone.length === 0) return;

		// @ts-ignore
		const result = await modal.showModal<CloneToUpsModalProps>(CloneToUpsModal, {
			title: m.clone_to_entity({ entity: m.ups() }),
			message: m.successfully_cloned_pals_to_entity({
				count: palsToClone.length,
				pals: c.pals,
				entity: c.universalPalStorage
			}),
			pals: palsToClone
		});

		if (!result) return;

		const { collectionId, tags, notes } = result;

		try {
			await upsState.cloneToUps(
				selectedPals,
				'gps',
				undefined,
				collectionId,
				tags.length > 0 ? tags : undefined,
				notes || undefined
			);

			toast.add(
				m.successfully_cloned_pals_to_entity({
					count: palsToClone.length,
					pals: c.pals,
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

	async function addAllPalsGps() {
		if (!appState.gps) return;
		// @ts-ignore
		await modal.showModal<string>(FillPalsModal, {
			title: m.fill_entity({ entity: m.global_pal_storage({ pal: m.pal({ count: 1 }) }) }),
			player: appState.selectedPlayer,
			target: 'gps'
		});
	}
</script>

{#if appState.loadingGps}
	<div class="flex h-full w-full items-center justify-center">
		<div class="flex flex-col items-center gap-4">
			<Spinner size="size-16" />
			<p class="text-surface-400">{m.loading_entity({ entity: m.gps() })}</p>
		</div>
	</div>
{:else if !appState.hasGpsAvailable && !appState.gpsLoaded}
	<div class="flex h-full w-full items-center justify-center">
		<div class="flex flex-col items-center gap-4">
			<p class="text-surface-400">{m.entity_not_available({ entity: c.globalPalStorage })}</p>
		</div>
	</div>
{:else}
	<div
		class="grid h-full w-full grid-cols-[25%_1fr] 2xl:grid-cols-[25%_1fr_20%]"
		{...additionalProps}
	>
		<div class="shrink-0 p-4">
			<div class="btn-group bg-surface-900 mb-2 w-full items-center rounded-sm p-1">
				<Tooltip
					position="right"
					label={m.add_all_pals_to_entity({ entity: m.gps(), pals: c.pals })}
				>
					<button class="btn hover:preset-tonal-secondary p-2" onclick={addAllPalsGps}>
						<CircleFadingPlus />
					</button>
				</Tooltip>
				<Tooltip>
					<button class="btn hover:preset-tonal-secondary p-2" onclick={handleSelectAll}>
						<ReplaceAll />
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

				{#if selectedPals.length >= 1}
					<Tooltip
						label={m.apply_preset_to_selected({ pals: m.pal({ count: selectedPals.length }) })}
					>
						<button class="btn hover:preset-tonal-secondary p-2" onclick={handleSelectPreset}>
							<Play />
						</button>
					</Tooltip>
					<Tooltip label={m.clone_selected_to_entity({ entity: m.ups(), pals: c.pals })}>
						<button class="btn hover:preset-tonal-secondary p-2" onclick={handleBulkCloneToUps}>
							<Upload />
						</button>
					</Tooltip>
					<Tooltip
						label={m.delete_selected_entity({ entity: m.pal({ count: selectedPals.length }) })}
					>
						<button class="btn hover:preset-tonal-secondary p-2" onclick={deleteSelectedPals}>
							<Trash />
						</button>
					</Tooltip>
					<Tooltip
						label={m.clear_selected_entity({ entity: m.pal({ count: selectedPals.length }) })}
					>
						<button
							class="btn hover:preset-tonal-secondary p-2"
							onclick={() => (selectedPals = [])}
						>
							<X />
						</button>
					</Tooltip>
				{/if}
			</div>
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
								<Tooltip label={m.sort_by_entity({ entity: m.name({ count: 1 }) })}>
									<button
										type="button"
										class={sortButtonClass('name')}
										onclick={() => toggleSort('name')}
									>
										<NameSortIcon />
									</button>
								</Tooltip>
								<Tooltip label={m.sort_by_paldeck()}>
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
									{@const localizedName = elementsData.elements[element].localized_name}
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
								<Tooltip label={c.human}>
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
			</Accordion>
		</div>

		<div>
			<!-- Pager -->
			<div class="mb-4 flex items-center justify-center space-x-4">
				<button class="rounded-sm px-4 py-2 font-bold" onclick={decrementPage}>
					<img src={staticIcons.qIcon} alt={m.previous()} class="h-10 w-10" />
				</button>

				<div class="flex space-x-2">
					{#each visiblePages as page}
						<TooltipButton
							class="h-8 w-8 rounded-full {page === currentPage
								? 'bg-primary-500 text-white'
								: 'bg-surface-800 hover:bg-gray-300'}"
							onclick={() => (currentPage = page)}
							popupLabel={`${m.box()} ${page}`}
						>
							{Math.floor(page)}
						</TooltipButton>
					{/each}
				</div>

				<button class="rounded-sm px-4 py-2 font-bold" onclick={incrementPage}>
					<img src={staticIcons.eIcon} alt={m.next()} class="h-10 w-10" />
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
								onMove={() => {}}
								onDelete={() => handleDeletePal(item.pal)}
								onAdd={() => handleAddPal(item.index)}
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
{/if}

<style>
	.pal-element-badge {
		width: 24px;
		height: 24px;
	}
</style>
