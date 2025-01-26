<script lang="ts">
	import PalBadge from '$components/badges/pal-badge/PalBadge.svelte';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { elementsData, palsData, getStats } from '$lib/data';
	import { getAppState, getSocketState, getModalState, getNavigationState } from '$states';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { Input, Tooltip, TooltipButton } from '$components/ui';
	import { PalSelectModal } from '$components/modals';
	import { type ElementType, type Pal, type PalData, MessageType } from '$types';
	import { assetLoader, debounce, calculateFilters, deepCopy, handleMaxOutPal } from '$utils';
	import { cn } from '$theme';
	import { staticIcons } from '$lib/constants';
	import {
		Search,
		GalleryVerticalEnd,
		ArrowDown01,
		ArrowDown10,
		ArrowDownAZ,
		ArrowDownZA,
		Plus,
		Copy,
		Ambulance,
		Trash,
		X,
		ArrowDownWideNarrow,
		ArrowDownNarrowWide,
		User,
		ReplaceAll,
		BicepsFlexed
	} from 'lucide-svelte';
	import Card from '$components/ui/card/Card.svelte';
	import { PalCard } from '$components';

	const PALS_PER_PAGE = 30;
	const TOTAL_SLOTS = 960;
	const VISIBLE_PAGE_BUBBLES = 16;

	type SortBy = 'name' | 'level' | 'paldeck-index' | 'slot-index';
	type SortOrder = 'asc' | 'desc';

	const appState = getAppState();
	const ws = getSocketState();
	const modal = getModalState();

	let { ...additionalProps } = $props<{
		[key: string]: any;
	}>();

	let searchQuery = $state('');
	let selectedFilter = $state('All');
	let currentPage = $state(1);
	let filteredPals: PalWithData[] = $state([]);
	let selectedPals: string[] = $state([]);
	let selectedPal: PalWithData | undefined = $state(undefined);
	let sortBy: SortBy = $state('slot-index');
	let sortOrder: SortOrder = $state('asc');

	type PalWithData = {
		id: string;
		pal: Pal;
		palData?: PalData;
	};

	let otomoContainer: Record<string, Pal> = $derived.by(() => {
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

	let totalPages = $derived(
		Math.ceil(
			searchQuery || selectedFilter !== 'All' || sortBy !== 'slot-index'
				? filteredPals.length
				: TOTAL_SLOTS
		) / PALS_PER_PAGE
	);
	let visiblePageStart = $derived(
		Math.max(
			1,
			Math.min(
				currentPage - Math.floor(VISIBLE_PAGE_BUBBLES / 2),
				totalPages - VISIBLE_PAGE_BUBBLES + 1
			)
		)
	);
	let visiblePageEnd = $derived(Math.min(visiblePageStart + VISIBLE_PAGE_BUBBLES - 1, totalPages));
	let visiblePages = $derived(
		Array.from({ length: visiblePageEnd - visiblePageStart + 1 }, (_, i) => visiblePageStart + i)
	);

	let currentPageItems = $derived.by(() => {
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

	let pals = $derived.by(() => {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return;
		console.log('Loading pals');
		const playerPals = Object.entries(appState.selectedPlayer.pals as Record<string, Pal>);
		const palBoxId = appState.selectedPlayer.pal_box_id;
		return playerPals
			.filter(([_, pal]) => pal.storage_id === palBoxId)
			.map(([id, pal]) => {
				const palData = palsData.pals[pal.character_key];
				return { id, pal, palData };
			});
	});

	let elementTypes = $derived(Object.keys(elementsData.elements));
	let elementIcons = $derived.by(() => {
		let elementIcons: Record<string, string> = {};
		for (const element of elementTypes) {
			const elementData = elementsData.elements[element];
			if (elementData) {
				elementIcons[element] = assetLoader.loadImage(
					`${ASSET_DATA_PATH}/img/elements/${elementData.icon}.png`
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

	function handleMoveToParty(pal: Pal) {
		if (appState.selectedPlayer) {
			const message = {
				type: MessageType.MOVE_PAL,
				data: {
					player_id: appState.selectedPlayer.uid,
					pal_id: pal.instance_id,
					container_id: appState.selectedPlayer.otomo_container_id
				}
			};
			ws.send(JSON.stringify(message));
		}
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

	async function filterPals() {
		if (!pals) return;
		filteredPals = pals.filter(({ pal, palData }) => {
			if (!palData) {
				console.log('No pal data for', pal);
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
			const matchesHuman = selectedFilter === 'human' ? !palData.is_pal : true;
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

	function formatNickname(nickname: string, type: 'clone' | 'new' = 'new') {
		if (type === 'new' && appState.settings.new_pal_prefix) {
			return `${appState.settings.new_pal_prefix} ${nickname}`;
		}
		if (type === 'clone' && appState.settings.clone_prefix) {
			return `${appState.settings.clone_prefix} ${nickname}`;
		}
		return nickname;
	}

	async function handleAddPal(target: 'party' | 'palbox', index: number | undefined = undefined) {
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		const result = await modal.showModal<[string, string] | undefined>(PalSelectModal, {
			title: 'Add a new Pal'
		});
		if (!result) return;
		const [selectedPal, nickname] = result;
		const palData = palsData.pals[selectedPal];
		const containerId =
			target === 'party'
				? appState.selectedPlayer.otomo_container_id
				: appState.selectedPlayer.pal_box_id;
		const message = {
			type: MessageType.ADD_PAL,
			data: {
				player_id: appState.selectedPlayer.uid,
				character_id: selectedPal,
				nickname: formatNickname(nickname || palData?.localized_name || selectedPal),
				container_id: containerId,
				storage_slot: index
			}
		};
		ws.send(JSON.stringify(message));
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

	async function cloneSelectedPal() {
		if (appState.selectedPlayer && appState.selectedPlayer.pals && selectedPal) {
			const pal = appState.selectedPlayer.pals[selectedPal.id];
			if (!pal) return;
			const clonedPal = deepCopy(pal);
			clonedPal.nickname = formatNickname(
				clonedPal.nickname || clonedPal.name || clonedPal.character_id,
				'clone'
			);
			const message = {
				type: MessageType.CLONE_PAL,
				data: clonedPal
			};
			ws.send(JSON.stringify(message));
		}
	}

	async function handleClonePal(pal: Pal) {
		const clonedPal = deepCopy(pal);
		clonedPal.nickname = formatNickname(
			clonedPal.nickname || clonedPal.name || clonedPal.character_id,
			'clone'
		);
		const message = {
			type: MessageType.CLONE_PAL,
			data: clonedPal
		};
		ws.send(JSON.stringify(message));
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

		const message = {
			type: MessageType.HEAL_PALS,
			data: [...selectedPals]
		};
		ws.send(JSON.stringify(message));

		Object.values(appState.selectedPlayer.pals).forEach(async (pal) => {
			if (selectedPals.includes(pal.instance_id)) {
				pal.hp = pal.max_hp;
				pal.sanity = 100;
				const palData = palsData.pals[pal.character_key];
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
			title: `Delete Pal${selectedPals.length > 1 ? 's' : ''}`,
			message: `Are you sure you want to delete the ${selectedPals.length} selected pal${selectedPals.length == 1 ? '' : 's'}?`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});

		if (appState.selectedPlayer && appState.selectedPlayer.pals && confirmed) {
			const message = {
				type: MessageType.DELETE_PALS,
				data: {
					player_id: appState.selectedPlayer.uid,
					pal_ids: [...selectedPals]
				}
			};
			ws.send(JSON.stringify(message));

			appState.selectedPlayer.pals = Object.fromEntries(
				Object.entries(appState.selectedPlayer.pals).filter(([id]) => !selectedPals.includes(id))
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
</script>

{#if appState.selectedPlayer}
	<div class="grid h-full w-full grid-cols-[25%_1fr]" {...additionalProps}>
		<div class="flex-shrink-0 p-4">
			<div class="btn-group bg-surface-900 mb-2 items-center rounded p-1">
				<Tooltip position="right">
					<button
						class="btn hover:preset-tonal-secondary p-2"
						onclick={() => handleAddPal('palbox')}
					>
						<Plus />
					</button>
					{#snippet popup()}
						Add a new pal to your Pal box
					{/snippet}
				</Tooltip>
				<Tooltip>
					<button
						class="btn hover:preset-tonal-secondary p-2"
						onclick={(event) => handleSelectAll(event)}
					>
						<ReplaceAll />
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
				{#if selectedPals.length === 1}
					<Tooltip>
						<button class="btn hover:preset-tonal-secondary p-2" onclick={cloneSelectedPal}>
							<Copy />
						</button>
						{#snippet popup()}
							Clone selected pal
						{/snippet}
					</Tooltip>
				{/if}
				{#if selectedPals.length >= 1}
					<Tooltip>
						<button class="btn hover:preset-tonal-secondary p-2" onclick={healSelectedPals}>
							<Ambulance />
						</button>
						{#snippet popup()}
							Heal selected pal(s)
						{/snippet}
					</Tooltip>
					<Tooltip>
						<button class="btn hover:preset-tonal-secondary p-2" onclick={maxSelectedPals}>
							<BicepsFlexed />
						</button>
						{#snippet popup()}
							Max out selected pal(s)
						{/snippet}
					</Tooltip>

					<Tooltip>
						<button class="btn hover:preset-tonal-secondary p-2" onclick={deleteSelectedPals}>
							<Trash />
						</button>
						{#snippet popup()}
							Delete selected pal(s)
						{/snippet}
					</Tooltip>
					<Tooltip>
						<button
							class="btn hover:preset-tonal-secondary p-2"
							onclick={() => (selectedPals = [])}
						>
							<X />
						</button>
						{#snippet popup()}
							Clear selected
						{/snippet}
					</Tooltip>
				{/if}
			</div>
			<Accordion classes="bg-surface-900" collapsible>
				<Accordion.Item value="filter" controlHover="hover:bg-secondary-500/25">
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
								<Tooltip>
									<button
										type="button"
										class={sortButtonClass('level')}
										onclick={() => toggleSort('level')}
									>
										<LevelSortIcon />
									</button>
									{#snippet popup()}
										Sort by level
									{/snippet}
								</Tooltip>
								<Tooltip>
									<button
										type="button"
										class={sortButtonClass('name')}
										onclick={() => toggleSort('name')}
									>
										<NameSortIcon />
									</button>
									{#snippet popup()}
										Sort by name
									{/snippet}
								</Tooltip>
								<Tooltip>
									<button
										type="button"
										class={sortButtonClass('paldeck-index')}
										onclick={() => toggleSort('paldeck-index')}
									>
										<PaldeckSortIcon />
									</button>
									{#snippet popup()}
										Sort by Paldeck #
									{/snippet}
								</Tooltip>
							</div>
						</div>
						<div>
							<legend class="font-bold">Element & Type</legend>
							<hr />
							<div class="mt-2 grid grid-cols-6">
								<Tooltip>
									<button class={elementClass('All')} onclick={() => (selectedFilter = 'All')}>
										<GalleryVerticalEnd />
									</button>
									{#snippet popup()}All pals{/snippet}
								</Tooltip>
								{#each [...elementTypes] as element}
									{@const localizedName = elementsData.elements[element].localized_name}
									<Tooltip>
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
										{#snippet popup()}
											<span>{localizedName}</span>
										{/snippet}
									</Tooltip>
								{/each}
								<Tooltip>
									<button
										type="button"
										class={sortAlphaClass}
										onclick={() => (selectedFilter = 'alpha')}
									>
										<img src={staticIcons.alphaIcon} alt="Alpha" class="pal-element-badge" />
									</button>
									{#snippet popup()}
										Alpha Pals
									{/snippet}
								</Tooltip>
								<Tooltip>
									<button
										type="button"
										class={sortLuckyClass}
										onclick={() => (selectedFilter = 'lucky')}
									>
										<img src={staticIcons.luckyIcon} alt="Alpha" class="pal-element-badge" />
									</button>
									{#snippet popup()}
										Lucky Pals
									{/snippet}
								</Tooltip>
								<Tooltip>
									<button
										type="button"
										class={sortHumanClass}
										onclick={() => (selectedFilter = 'human')}
									>
										<User />
									</button>
									{#snippet popup()}
										Humans
									{/snippet}
								</Tooltip>
								<Tooltip>
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
									{#snippet popup()}
										Predator Pals
									{/snippet}
								</Tooltip>
								<Tooltip>
									<button
										type="button"
										class={sortOilrigClass}
										onclick={() => (selectedFilter = 'oilrig')}
									>
										<img src={staticIcons.oilrigIcon} alt="Oil Rig" class="pal-element-badge" />
									</button>
									{#snippet popup()}
										Oil Rig Pals
									{/snippet}
								</Tooltip>
								<Tooltip>
									<button
										type="button"
										class={sortSummonClass}
										onclick={() => (selectedFilter = 'summon')}
									>
										<img src={staticIcons.altarIcon} alt="Summoned" class="pal-element-badge" />
									</button>
									{#snippet popup()}
										Summoned Pals
									{/snippet}
								</Tooltip>
							</div>
						</div>
					{/snippet}
				</Accordion.Item>
			</Accordion>
			<div class="mt-2 flex">
				<Card rounded="rounded-none" class="w-full p-4">
					<div class="flex flex-col space-y-2">
						<h6 class="h6 mr-4">Party</h6>
						{#each Object.values(otomoContainer) as pal, index}
							<PalCard
								bind:pal={otomoContainer[pal.instance_id]}
								bind:selected={selectedPals}
								onSelect={handlePalSelect}
								onMove={() => handleMoveToPalbox(pal)}
								onDelete={() => handleDeletePal(pal)}
								onAdd={() => handleAddPal('party', index)}
								onClone={() => handleClonePal(pal)}
							/>
						{/each}
					</div>
				</Card>
			</div>
		</div>

		<div>
			<!-- Pager -->
			<div class="mb-4 flex items-center justify-center space-x-4">
				<button class="rounded px-4 py-2 font-bold" onclick={decrementPage}>
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
							{page}
						</TooltipButton>
					{/each}
				</div>

				<button class="rounded px-4 py-2 font-bold" onclick={incrementPage}>
					<img src={staticIcons.eIcon} alt="Next" class="h-10 w-10" />
				</button>
			</div>

			<div class="overflow-hidden">
				<div class="grid grid-cols-6 gap-4 p-4">
					{#each currentPageItems as item (item.pal.instance_id)}
						{#if item.pal.character_id !== 'None' || (!searchQuery && selectedFilter === 'All' && sortBy === 'slot-index')}
							<PalBadge
								bind:pal={item.pal}
								bind:selected={selectedPals}
								onSelect={handlePalSelect}
								onMove={() => handleMoveToParty(item.pal)}
								onDelete={() => handleDeletePal(item.pal)}
								onAdd={() => handleAddPal('palbox', item.pal.storage_slot)}
								onClone={() => handleClonePal(item.pal)}
							/>
						{/if}
					{/each}
				</div>
			</div>
		</div>
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
