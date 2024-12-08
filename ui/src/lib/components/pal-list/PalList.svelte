<script lang="ts">
	import { cn } from '$theme';
	import {
		MessageType,
		PalGender,
		type ElementType,
		type Pal,
		type PalData,
		type Player
	} from '$types';
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
		Users
	} from 'lucide-svelte';
	import { assetLoader, debounce, deepCopy } from '$utils';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { getAppState, getSocketState, getModalState, getNavigationState } from '$states';
	import { HealthBadge } from '$components';
	import type { Component } from 'svelte';

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
	let elementTypes: string[] = $state([]);
	let elementBadges: Record<string, string> = $state({});
	let elementIcons: Record<string, string> = $state({});
	let genderIcons: Record<PalGender, string> = $state({
		[PalGender.MALE]: '',
		[PalGender.FEMALE]: ''
	});
	let pals: PalWithData[] = $state([]);
	let filteredPals: PalWithData[] = $state([]);
	let selectedPals: PalWithData[] = $state([]);
	let selectedPal: PalWithData | undefined = $state(undefined);
	let palMenuIcons: Record<string, string> = $state({});
	let sortBy: SortBy = $state('slot-index');
	let sortOrder: SortOrder = $state('asc');
	let alphaIcon: string = $state('');
	let foodIcon: string = $state('');
	let hpIcon: string = $state('');
	let containerRef: HTMLDivElement;
	let listWrapperStyle = $state('');
	let selectAll: boolean = $state(false);

	const sortLuckyClass = $derived(
		cn('btn', selectedFilter === 'lucky' ? 'bg-secondary-500/25' : '')
	);
	const sortAlphaClass = $derived(
		cn('btn', selectedFilter === 'alpha' ? 'bg-secondary-500/25' : '')
	);

	const sortButtonClass = (currentSortBy: SortBy) =>
		cn('btn', sortBy === currentSortBy ? 'bg-secondary-500/25' : '');

	const elementClass = (element: string) =>
		cn('btn', selectedFilter === element ? 'bg-secondary-500/25' : '');

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

	async function getPalsInfo() {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return;

		const playerPals = Object.entries(appState.selectedPlayer.pals as Record<string, Pal>);
		const palBoxId = appState.selectedPlayer.pal_box_id;
		pals = await Promise.all(
			playerPals
				.filter(([_, pal]) => pal.storage_id === palBoxId)
				.map(async ([id, pal]) => {
					const palData = await palsData.getPalInfo(pal.character_id);
					await getStats(pal, appState.selectedPlayer as Player);
					return { id, pal, palData };
				})
		);
	}

	async function filterPals() {
		if (!pals) return;
		selectedPals = [];
		filteredPals = pals.filter(({ pal, palData }) => {
			const matchesSearch =
				pal.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
				pal.nickname?.toLowerCase().includes(searchQuery.toLowerCase());
			const matchesElement =
				selectedFilter === 'All' ||
				selectedFilter === 'alpha' ||
				selectedFilter === 'lucky' ||
				(palData &&
					palData.element_types &&
					palData.element_types
						.map((e: ElementType) => e.toString()!.toLowerCase())
						.includes(selectedFilter.toLowerCase()));
			const matchesAlpha = selectedFilter === 'alpha' ? pal.is_boss : true;
			const matchesLucky = selectedFilter === 'lucky' ? pal.is_lucky : true;
			return matchesSearch && matchesElement && matchesAlpha && matchesLucky;
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

	async function loadStaticIcons() {
		const iconPath = `${ASSET_DATA_PATH}/img/icons/Alpha.png`;
		alphaIcon = await assetLoader.loadImage(iconPath);

		const foodPath = `${ASSET_DATA_PATH}/img/icons/Food.png`;
		foodIcon = await assetLoader.loadImage(foodPath);

		const hpPath = `${ASSET_DATA_PATH}/img/icons/Heart.png`;
		hpIcon = await assetLoader.loadImage(hpPath);

		const malePath = `${ASSET_DATA_PATH}/img/icons/${PalGender.MALE.toLowerCase()}.svg`;
		genderIcons[PalGender.MALE] = await assetLoader.loadSvg(malePath);

		const femalePath = `${ASSET_DATA_PATH}/img/icons/${PalGender.FEMALE.toLowerCase()}.svg`;
		genderIcons[PalGender.FEMALE] = await assetLoader.loadSvg(femalePath);
	}

	// @ts-ignore
	let LevelSortIcon: Component = $state(ArrowDown01);
	$effect(() => {
		if (sortBy !== 'level') {
			// @ts-ignore
			LevelSortIcon = ArrowDown01;
		} else {
			// @ts-ignore
			LevelSortIcon = sortOrder === 'asc' ? ArrowDown01 : ArrowDown10;
		}
	});

	// @ts-ignore
	let NameSortIcon: Component = $state(ArrowDownAZ);
	$effect(() => {
		if (sortBy !== 'name') {
			// @ts-ignore
			NameSortIcon = ArrowDownAZ;
		} else {
			// @ts-ignore
			NameSortIcon = sortOrder === 'asc' ? ArrowDownAZ : ArrowDownZA;
		}
	});

	// @ts-ignore
	let PaldeckSortIcon: Component = $state(ArrowDownWideNarrow);
	$effect(() => {
		if (sortBy !== 'paldeck-index') {
			// @ts-ignore
			PaldeckSortIcon = ArrowDownWideNarrow;
		} else {
			// @ts-ignore
			PaldeckSortIcon = sortOrder === 'asc' ? ArrowDownWideNarrow : ArrowDownNarrowWide;
		}
	});

	async function loadElementTypes() {
		elementTypes = await elementsData.getAllElementTypes();
	}

	async function loadElementIcons() {
		for (const elementType of elementTypes) {
			const elementObj = await elementsData.searchElement(elementType);
			if (elementObj) {
				const badgePath = `${ASSET_DATA_PATH}/img/elements/${elementObj.badge_icon}.png`;
				const iconPath = `${ASSET_DATA_PATH}/img/elements/${elementObj.icon}.png`;
				try {
					elementBadges[elementType] = await assetLoader.loadImage(badgePath, true);
					elementIcons[elementType] = await assetLoader.loadImage(iconPath, true);
				} catch (error) {
					console.error(`Failed to load icon for ${elementType}:`, error);
				}
			}
		}
	}

	function getElementIcon(elementType: string): string | undefined {
		if (elementIcons[elementType]) return elementIcons[elementType];
		return elementIcons[elementType];
	}

	async function getPalMenuIcon(palId: string): Promise<string | undefined> {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return undefined;
		const pal = appState.selectedPlayer.pals[palId];
		if (!pal) return undefined;
		if (palMenuIcons[pal.character_id]) return palMenuIcons[pal.character_id];
		const palImgName = pal.name.toLowerCase().replaceAll(' ', '_');
		const icon_path = `${ASSET_DATA_PATH}/img/pals/menu/${palImgName}_menu.png`;
		const icon = await assetLoader.loadImage(icon_path, true);
		palMenuIcons[pal.character_id] = icon;
		return icon;
	}

	async function handleAddPal() {
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		const [selectedPal, nickname] = await modal.showModal<string>(PalSelectModal, {
			title: 'Add a new Pal'
		});
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
		console.log('Sort by slot index');
		filteredPals = filteredPals.sort((a, b) =>
			sortOrder === 'asc'
				? a.pal.storage_slot - b.pal.storage_slot
				: b.pal.storage_slot - a.pal.storage_slot
		);
	}

	async function sortByPaldeckIndex() {
		const palInfoPromises = filteredPals.map((pal) => palsData.getPalInfo(pal.pal.character_id));
		const palInfos = await Promise.all(palInfoPromises);

		const palsWithInfo = filteredPals.map((pal, index) => [pal, palInfos[index]]);

		palsWithInfo.sort((a, b) => {
			const indexA = (a[1] as PalData)?.pal_deck_index ?? Infinity;
			const indexB = (b[1] as PalData)?.pal_deck_index ?? Infinity;
			return sortOrder === 'asc' ? indexA - indexB : indexB - indexA;
		});

		filteredPals = palsWithInfo.map((pair) => pair[0] as PalWithData);
	}

	function getGenderIcon(gender: PalGender): string | undefined {
		return genderIcons[gender];
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
		const palData = await palsData.getPalInfo(pal.character_id);
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
		loadStaticIcons();
		loadElementTypes();
		calculateHeight();
	});

	$effect(() => {
		if (elementTypes.length > 0) {
			loadElementIcons();
		}
	});

	$effect(() => {
		if (appState.selectedPlayer && appState.selectedPlayer.pals) {
			getPalsInfo();
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
								<Tooltip>
									{#await getElementIcon(element) then icon}
										{#if icon}
											<button
												class={elementClass(element)}
												onclick={() => (selectedFilter = element)}
												aria-label={element}
											>
												<enhanced:img src={icon} alt={element} class="pal-element-badge"
												></enhanced:img>
											</button>
										{/if}
									{/await}
									{#snippet popup()}
										<span>{element}</span>
									{/snippet}
								</Tooltip>
							{/each}
							<Tooltip>
								<button
									type="button"
									class={sortAlphaClass}
									onclick={() => (selectedFilter = 'alpha')}
								>
									{#if alphaIcon}
										<enhanced:img src={alphaIcon} alt="Aplha" class="pal-element-badge"
										></enhanced:img>
									{/if}
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
						<div>
							<span class="font-bold">Lvl {p.pal.level}</span>
						</div>
						<div class="relative justify-start">
							{#if p.pal.is_boss}
								{#if alphaIcon}
									<div class="absolute -left-2 -top-1 h-5 w-5">
										<enhanced:img src={alphaIcon} alt="Aplha" class="pal-element-badge"
										></enhanced:img>
									</div>
								{/if}
							{/if}
							{#if p.pal.is_lucky}
								<div class="absolute -left-2 -top-1 h-5 w-5">✨</div>
							{/if}
							{#await getPalMenuIcon(p.pal.instance_id) then icon}
								{#if icon}
									<enhanced:img src={icon} alt={p.pal.name} class="h-8 w-8"></enhanced:img>
								{/if}
							{/await}
							{#await getGenderIcon(p.pal.gender) then icon}
								{#if icon}
									{@const color =
										p.pal.gender == PalGender.MALE ? 'text-primary-300' : 'text-tertiary-300'}
									<div class={cn('absolute -right-4 -top-1 h-5 w-5', color)}>
										{@html icon}
									</div>
								{/if}
							{/await}
						</div>
						<div class="ml-4 flex flex-row justify-start">
							<div>{p.pal.nickname || p.pal.name}</div>
						</div>
						<div class="flex justify-end">
							{#if p.palData}
								{#each p.palData.element_types as elementType}
									{#await getElementIcon(elementType) then icon}
										{#if icon}
											<enhanced:img src={icon} alt={elementType} class="pal-element-badge"
											></enhanced:img>
										{/if}
									{/await}
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
