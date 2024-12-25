<script lang="ts">
	import { cn } from '$theme';
	import { MessageType, type ElementType, type Pal, type PalData, type Player } from '$types';
	import { elementsData, palsData, getStats } from '$lib/data';
	import { Input, Tooltip, List, ContextMenu } from '$components/ui';
	import { PalSelectModal } from '$components/modals';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
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
		Users,
		User
	} from 'lucide-svelte';
	import { assetLoader, debounce, deepCopy } from '$utils';
	import { ASSET_DATA_PATH, staticIcons } from '$lib/constants';
	import { getAppState, getSocketState, getModalState, getNavigationState } from '$states';
	import { HealthBadge } from '$components';

	type SortBy = 'name' | 'level' | 'paldeck-index' | 'slot-index';
	type SortOrder = 'asc' | 'desc';

	const appState = getAppState();
	const ws = getSocketState();
	const modal = getModalState();
	const nav = getNavigationState();

	let { ...additionalProps } = $props<{
		[key: string]: any;
	}>();

	type PalWithData = {
		id: string;
		pal: Pal;
		palData?: PalData;
	};

	let searchQuery = $state('');
	let selectedFilter = $state('All');

	let filteredPals: PalWithData[] = $state([]);
	let selectedPals: PalWithData[] = $state([]);
	let selectedPal: PalWithData | undefined = $state(undefined);
	let sortBy: SortBy = $state('slot-index');
	let sortOrder: SortOrder = $state('asc');
	let containerRef: HTMLDivElement;
	let listWrapperStyle = $state('');
	let selectAll: boolean = $state(false);

	const sortLuckyClass = $derived(
		cn('btn', selectedFilter === 'lucky' ? 'bg-secondary-500/25' : '')
	);
	const sortAlphaClass = $derived(
		cn('btn', selectedFilter === 'alpha' ? 'bg-secondary-500/25' : '')
	);
	const sortHumanClass = $derived(
		cn('btn', selectedFilter === 'human' ? 'bg-secondary-500/25' : '')
	);

	const sortButtonClass = (currentSortBy: SortBy) =>
		cn('btn', sortBy === currentSortBy ? 'bg-secondary-500/25' : '');

	const elementClass = (element: string) =>
		cn('btn', selectedFilter === element ? 'bg-secondary-500/25' : '');

	let pals = $derived.by(() => {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return;
		const playerPals = Object.entries(appState.selectedPlayer.pals as Record<string, Pal>);
		const palBoxId = appState.selectedPlayer.pal_box_id;
		return playerPals
			.filter(([_, pal]) => pal.storage_id === palBoxId)
			.map(([id, pal]) => {
				const palData = palsData.pals[pal.character_id];
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

	const debouncedFilterPals = debounce(filterPals, 300);

	function calculateHeight() {
		if (containerRef) {
			const rect = containerRef.getBoundingClientRect();
			const windowHeight = window.innerHeight;
			const listHeight = windowHeight - rect.top - 110;
			listWrapperStyle = `height: ${listHeight}px;`;
		}
	}

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

	async function filterPals() {
		if (!pals) return;
		selectedPals = [];
		filteredPals = pals.filter(({ pal, palData }) => {
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
			return matchesSearch && matchesElement && matchesAlpha && matchesLucky && matchesHuman;
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

	function handlePalSelect(p: PalWithData) {
		if (appState.selectedPlayer && appState.selectedPlayer.pals) {
			appState.selectedPal = appState.selectedPlayer.pals[p.pal.instance_id];
			nav.activeTab = 'pal';
		}
	}

	async function getPalMenuIcon(palId: string): Promise<string | undefined> {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return undefined;
		const pal = appState.selectedPlayer.pals[palId];
		if (!pal) return undefined;
		const palData = palsData.pals[pal.character_id];
		if (palData && palData.is_pal) {
			return assetLoader.loadMenuImage(pal.character_id);
		} else if (palData && !palData.is_pal) {
			return assetLoader.loadMenuImage(pal.character_id, false);
		} else {
			return staticIcons.sadIcon;
		}
	}

	async function handleAddPal() {
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		const result = await modal.showModal<[string, string] | undefined>(PalSelectModal, {
			title: 'Add a new Pal'
		});
		if (!result) return;
		const [selectedPal, nickname] = result;
		const palData = await palsData.getPalInfo(selectedPal);
		const message = {
			type: MessageType.ADD_PAL,
			data: {
				player_id: appState.selectedPlayer.uid,
				pal_code_name: selectedPal,
				nickname: nickname || `[New] ${palData?.localized_name}`,
				container_id: appState.selectedPlayer.pal_box_id
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
		const palInfos = filteredPals.map((p) => palsData.pals[p.pal.character_id]);
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
			clonedPal.nickname = clonedPal.nickname
				? `[Clone] ${clonedPal.nickname}`
				: `[Clone] ${clonedPal.name}`;
			const message = {
				type: MessageType.CLONE_PAL,
				data: clonedPal
			};
			ws.send(JSON.stringify(message));
		}
	}

	async function handleClonePal(pal: Pal) {
		const clonedPal = deepCopy(pal);
		clonedPal.nickname = clonedPal.nickname
			? `[Clone] ${clonedPal.nickname}`
			: `[Clone] ${clonedPal.name}`;
		const message = {
			type: MessageType.CLONE_PAL,
			data: clonedPal
		};
		ws.send(JSON.stringify(message));
	}

	async function healSelectedPals() {
		if (appState.selectedPlayer && appState.selectedPlayer.pals) {
			const selectedPalIds = selectedPals.map((p) => p.pal.instance_id);
			const message = {
				type: MessageType.HEAL_PALS,
				data: selectedPalIds
			};
			ws.send(JSON.stringify(message));
			Object.values(appState.selectedPlayer.pals).forEach(async (pal) => {
				if (selectedPalIds.includes(pal.instance_id)) {
					pal.hp = pal.max_hp;
					pal.sanity = 100;
					const palData = await palsData.getPalInfo(pal.character_id);
					if (palData) {
						pal.stomach = palData.max_full_stomach;
					}
				}
			});
		}
		selectedPals = [];
		selectAll = false;
	}

	async function handleHealPal(pal: Pal) {
		const message = {
			type: MessageType.HEAL_PALS,
			data: [pal.instance_id]
		};
		ws.send(JSON.stringify(message));
		pal.hp = pal.max_hp;
		pal.sanity = 100;
		const palData = palsData.pals[pal.character_id];
		if (palData) {
			pal.stomach = palData.max_full_stomach;
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
		if (appState.selectedPlayer && appState.selectedPlayer.pals && confirmed) {
			const selectedPalIds = selectedPals.map((p) => p.pal.instance_id);
			const data = {
				player_id: appState.selectedPlayer.uid,
				pal_ids: selectedPalIds
			};
			const message = {
				type: MessageType.DELETE_PALS,
				data
			};
			ws.send(JSON.stringify(message));
			appState.selectedPlayer.pals = Object.fromEntries(
				Object.entries(appState.selectedPlayer.pals).filter(([id]) => !selectedPalIds.includes(id))
			);
		}
		selectedPals = [];
		selectAll = false;
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

	$effect(() => {
		calculateHeight();
	});

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
		window.addEventListener('resize', calculateHeight);
		return () => {
			window.removeEventListener('resize', calculateHeight);
		};
	});

	$effect(() => {
		if (pals && appState.selectedPlayer) {
			pals.forEach((p) => {
				getStats(p.pal, appState.selectedPlayer!);
			});
		}
	});
</script>

<div class="flex h-full w-full flex-col space-y-2" bind:this={containerRef} {...additionalProps}>
	<div class="flex-shrink-0">
		<div class="btn-group bg-surface-900 mb-2 items-center rounded p-1">
			<Tooltip position="right">
				<button class="btn hover:preset-tonal-secondary p-2" onclick={handleAddPal}>
					<Plus />
				</button>
				{#snippet popup()}
					Add a new pal to your Pal box
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
					<button class="btn hover:preset-tonal-secondary p-2" onclick={deleteSelectedPals}>
						<Trash />
					</button>
					{#snippet popup()}
						Delete selected pal(s)
					{/snippet}
				</Tooltip>
				<Tooltip>
					<button class="btn hover:preset-tonal-secondary p-2" onclick={() => (selectedPals = [])}>
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
									<img src={staticIcons.alphaIcon} alt="Aplha" class="pal-element-badge" />
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
									✨
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
						</div>
					</div>
				{/snippet}
			</Accordion.Item>
		</Accordion>
	</div>
	<div class="overflow-hidden" style={listWrapperStyle}>
		<List
			baseClass="h-full"
			bind:items={filteredPals}
			bind:selectedItems={selectedPals}
			bind:selectedItem={selectedPal}
			bind:selectAll
			onselect={handlePalSelect}
		>
			{#snippet listHeader()}
				<div>
					<span class="font-bold">Level</span>
				</div>
				<div class="bg-surface-900 z-50 w-[55px]"></div>
				<div class="flex justify-start">
					<span class="font-bold">Name</span>
				</div>
				<div class="flex justify-end">
					<span class="font-bold">Element</span>
				</div>
			{/snippet}
			{#snippet listItem(p)}
				{@const menuItems = [
					{ label: 'Move to Party', onClick: () => handleMoveToParty(p.pal), icon: Users },
					{ label: 'Clone Pal', onClick: () => handleClonePal(p.pal), icon: Copy },
					{ label: 'Heal Pal', onClick: () => handleHealPal(p.pal), icon: Ambulance },
					{ label: 'Delete Pal', onClick: () => handleDeletePal(p.pal), icon: Trash }
				]}
				<ContextMenu items={menuItems} baseClass="w-full" position="bottom">
					<div class="grid w-full grid-cols-[55px_auto_1fr_auto] gap-2">
						<div class="flex items-end space-x-1">
							<span class="text-surface-300 text-xs">Lvl</span>
							<span class="font-bold">{p.pal.level}</span>
						</div>
						<div class="relative justify-start">
							{#if p.pal.is_boss}
								<div class="absolute -left-2 -top-1 h-5 w-5">
									<img src={staticIcons.alphaIcon} alt="Aplha" class="pal-element-badge" />
								</div>
							{/if}
							{#if p.pal.is_lucky}
								<div class="absolute -left-2 -top-1 h-5 w-5">✨</div>
							{/if}
							{#await getPalMenuIcon(p.pal.instance_id) then icon}
								<img src={icon} alt={p.pal.name} class="h-8 w-8" />
							{/await}
							<div class="absolute -right-4 -top-1 h-5 w-5">
								<img
									src={assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/${p.pal.gender}.png`)}
									alt={p.pal.gender}
								/>
							</div>
						</div>
						<div class="ml-4 flex flex-row justify-start">
							<div>{p.pal.nickname || p.pal.name}</div>
						</div>
						<div class="flex justify-end">
							{#if p.palData}
								{#each p.palData.element_types as elementType}
									<img
										src={elementIcons[elementType]}
										alt={elementType}
										class="pal-element-badge"
									/>
								{/each}
							{/if}
						</div>
					</div>
				</ContextMenu>
			{/snippet}
			{#snippet listItemPopup(p)}
				<div class="flex w-[450px] flex-col">
					<HealthBadge bind:pal={p.pal} player={appState.selectedPlayer} />
					<span>{p.palData?.description}</span>
				</div>
			{/snippet}
		</List>
	</div>
</div>

<style>
	.pal-element-badge {
		width: 24px;
		height: 24px;
	}
</style>
