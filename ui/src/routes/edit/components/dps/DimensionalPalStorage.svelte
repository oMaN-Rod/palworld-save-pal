<script lang="ts">
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { elementsData, palsData, presetsData } from '$lib/data';
	import { getAppState, getModalState, getToastState } from '$states';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { Card, Input, Tooltip, TooltipButton } from '$components/ui';
	import {
		PalSelectModal,
		FillPalsModal,
		NumberInputModal,
		PalPresetSelectModal
	} from '$components/modals';
	import { type ElementType, type Pal, type PalData, MessageType } from '$types';
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
		Play
	} from 'lucide-svelte';
	import { PalBadge, PalContainerStats } from '$components';
	import { send } from '$lib/utils/websocketUtils';
	import type { ValueChangeDetails } from '@zag-js/accordion';

	const PALS_PER_PAGE = 30;
	const TOTAL_SLOTS = 9600;
	const VISIBLE_PAGE_BUBBLES = 16;

	type SortBy = 'name' | 'level' | 'paldeck-index' | 'slot-index';
	type SortOrder = 'asc' | 'desc';

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();

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

	let pals = $derived.by(() => {
		if (!appState.selectedPlayer || !appState.selectedPlayer.dps) return;
		const playerPals = Object.entries(appState.selectedPlayer.dps).filter(
			([_, pal]) => pal && pal.character_id !== 'None'
		);
		return playerPals.map(([i, pal]) => {
			const palData = palsData.getByKey(pal.character_key);
			return { id: pal.instance_id, index: i as unknown as number, pal, palData };
		});
	});

	let elementTypes = $derived(Object.keys(elementsData.elements));
	let elementIcons = $derived.by(() => {
		let elementIcons: Record<string, string> = {};
		for (const element of elementTypes) {
			const elementData = elementsData.getByKey(element);
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
			title: 'Select preset',
			selectedPals: selectedPalsData
		});
		if (!result) return;

		const presetProfile = presetsData.presetProfiles[result];

		selectedPals.forEach((id) => {
			const palWithData = pals?.find((p) => p.id === id);
			if (palWithData) {
				applyPalPreset(palWithData.pal, presetProfile, appState.selectedPlayer!);
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
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		const result = await modal.showModal<[string, string] | undefined>(PalSelectModal, {
			title: `Add a new Pal to your DPS`
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
			? 9600 - Object.values(appState.selectedPlayer!.dps).length
			: 0;
		if (maxClones === 0) {
			toast.add('There are no slots available in your Dimensional Pal Storage.', 'Error', 'error');
			return;
		}
		// @ts-ignore
		const result = await modal.showModal<number>(NumberInputModal, {
			title: 'How many clones?',
			message: `There are ${maxClones} slots available in your Dimensional Pal Storage.`,
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
		const palInfos = filteredPals.map((p) => palsData.getByKey(p.pal.character_key));
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
			title: `Delete Pal${selectedPals.length > 1 ? 's' : ''}`,
			message: `Are you sure you want to delete the ${selectedPals.length} selected pal${selectedPals.length == 1 ? '' : 's'}?`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});

		if (appState.selectedPlayer && appState.selectedPlayer.dps && confirmed) {
			const palIndexes = Object.entries(appState.selectedPlayer.dps)
				.filter(([_, pal]) => selectedPals.includes(pal.instance_id))
				.map(([index]) => index);
			send(MessageType.DELETE_DPS_PALS, {
				player_id: appState.selectedPlayer.uid,
				pal_indexes: palIndexes
			});

			appState.selectedPlayer.dps = Object.fromEntries(
				Object.entries(appState.selectedPlayer.dps).filter(([idx, _]) => !palIndexes.includes(idx))
			);
		}

		selectedPals = [];
	}

	async function handleDeletePal(pal: Pal) {
		const confirmed = await modal.showConfirmModal({
			title: 'Delete Pal',
			message: `Are you sure you want to delete ${pal.nickname || pal.name}?`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});
		if (appState.selectedPlayer && appState.selectedPlayer.dps && confirmed) {
			const palIndex = Object.entries(appState.selectedPlayer.dps).find(
				([_, p]) => p.instance_id === pal.instance_id
			);
			send(MessageType.DELETE_DPS_PALS, {
				player_id: appState.selectedPlayer.uid,
				pal_indexes: [palIndex![0]]
			});
			appState.selectedPlayer.dps = Object.fromEntries(
				Object.entries(appState.selectedPlayer.dps).filter(
					([_, p]) => p.instance_id !== pal.instance_id
				)
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
		if (appState.selectedPlayer && appState.selectedPlayer.dps) {
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

	async function addAllPalsDps() {
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		await modal.showModal<string>(FillPalsModal, {
			title: 'Fill Dimensional Pal Storage',
			player: appState.selectedPlayer,
			target: 'dps'
		});
	}
</script>

{#if appState.selectedPlayer}
	<div
		class="grid h-full w-full grid-cols-[25%_1fr] 2xl:grid-cols-[25%_1fr_20%]"
		{...additionalProps}
	>
		<div class="shrink-0 p-4">
			<div class="btn-group bg-surface-900 mb-2 w-full items-center rounded-sm p-1">
				<Tooltip position="right" label="Add all pals to your Pal Box">
					<button class="btn hover:preset-tonal-secondary p-2" onclick={addAllPalsDps}>
						<CircleFadingPlus class="h-4 w-4" />
					</button>
				</Tooltip>
				<Tooltip>
					<button class="btn hover:preset-tonal-secondary p-2" onclick={handleSelectAll}>
						<ReplaceAll class="h-4 w-4" />
					</button>
					{#snippet popup()}
						<div class="flex flex-col">
							<span>Select all in</span>
							<div class="grid grid-cols-[auto_1fr] gap-1">
								<img src={staticIcons.leftClickIcon} alt="Left Click" class="h-6 w-6" />
								<span class="text-sm">pal box</span>
								<div class="flex">
									<img src={staticIcons.ctrlIcon} alt="Ctrl" class="h-6 w-6" />
									<img src={staticIcons.leftClickIcon} alt="Left Click" class="h-6 w-6" />
								</div>
								<span class="text-sm">pal box + party</span>
							</div>
						</div>
					{/snippet}
				</Tooltip>

				{#if selectedPals.length >= 1}
					<Tooltip label="Apply preset to selected pal(s)">
						<button class="btn hover:preset-tonal-secondary p-2" onclick={handleSelectPreset}>
							<Play class="h-4 w-4" />
						</button>
					</Tooltip>
					<Tooltip label="Delete selected pal(s)">
						<button class="btn hover:preset-tonal-secondary p-2" onclick={deleteSelectedPals}>
							<Trash class="h-4 w-4" />
						</button>
					</Tooltip>
					<Tooltip label="Clear selected pal(s)">
						<button
							class="btn hover:preset-tonal-secondary p-2"
							onclick={() => (selectedPals = [])}
						>
							<X class="h-4 w-4" />
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
						<span class="font-bold">Filter & Sort</span>
					{/snippet}
					{#snippet panel()}
						<Input
							type="text"
							inputClass="w-full"
							placeholder="Search by name or nickname"
							bind:value={searchQuery}
						/>
						<div>
							<legend class="font-bold">Sort</legend>
							<hr />
							<div class="grid grid-cols-6">
								<Tooltip label="Sort by level">
									<button
										type="button"
										class={sortButtonClass('level')}
										onclick={() => toggleSort('level')}
									>
										<LevelSortIcon />
									</button>
								</Tooltip>
								<Tooltip label="Sort by name">
									<button
										type="button"
										class={sortButtonClass('name')}
										onclick={() => toggleSort('name')}
									>
										<NameSortIcon />
									</button>
								</Tooltip>
								<Tooltip label="Sort by Paldeck #">
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
							<legend class="font-bold">Element & Type</legend>
							<hr />
							<div class="mt-2 grid grid-cols-4 2xl:grid-cols-6">
								<Tooltip>
									<button class={elementClass('All')} onclick={() => (selectedFilter = 'All')}>
										<GalleryVerticalEnd />
									</button>
									{#snippet popup()}All pals{/snippet}
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
								<Tooltip label="Alpha Pals">
									<button
										type="button"
										class={sortAlphaClass}
										onclick={() => (selectedFilter = 'alpha')}
									>
										<img src={staticIcons.alphaIcon} alt="Alpha" class="pal-element-badge" />
									</button>
								</Tooltip>
								<Tooltip label="Lucky Pals">
									<button
										type="button"
										class={sortLuckyClass}
										onclick={() => (selectedFilter = 'lucky')}
									>
										<img src={staticIcons.luckyIcon} alt="Alpha" class="pal-element-badge" />
									</button>
								</Tooltip>
								<Tooltip label="Humans">
									<button
										type="button"
										class={sortHumanClass}
										onclick={() => (selectedFilter = 'human')}
									>
										<User />
									</button>
								</Tooltip>
								<Tooltip label="Predator Pals">
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
								<Tooltip label="Oil Rig Pals">
									<button
										type="button"
										class={sortOilrigClass}
										onclick={() => (selectedFilter = 'oilrig')}
									>
										<img src={staticIcons.oilrigIcon} alt="Oil Rig" class="pal-element-badge" />
									</button>
								</Tooltip>
								<Tooltip label="Summoned Pals">
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
						<span class="font-bold">Stats</span>
					{/snippet}
					{#snippet panel()}
						{#if pals && pals.length > 0}
							<PalContainerStats {pals} {elementTypes} />
						{:else}
							<div>No pals data available</div>
						{/if}
					{/snippet}
				</Accordion.Item>
			</Accordion>
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
								onMove={() => {}}
								onDelete={() => handleDeletePal(item.pal)}
								onAdd={() => handleAddPal(item.index)}
								onClone={() => handleClonePal(item.pal)}
							/>
						{/if}
					{/each}
				</div>
			</div>
		</div>

		{#if pals && pals.length > 0}
			<Card class="mr-2 hidden h-[430px] 2xl:block">
				<PalContainerStats {pals} {elementTypes} />
			</Card>
		{:else}
			<Card class="mr-2 hidden h-[430px] 2xl:block">
				<div>No pals data available</div>
			</Card>
		{/if}
	</div>
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">Select a Player to view Pal Box</h2>
	</div>
{/if}

<style>
	.pal-element-badge {
		width: 24px;
		height: 24px;
	}
</style>
