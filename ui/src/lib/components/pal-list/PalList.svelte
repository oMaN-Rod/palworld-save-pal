<script lang="ts">
	import { cn } from '$theme';
	import { MessageType, PalGender, type Pal, type Player } from '$types';
	import { elementsData, palsData, getStats } from '$lib/data';
	import { Input, Tooltip, List } from '$components/ui';
	import { PalSelectModal } from '$components/modals';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import {
		Search,
		GalleryVerticalEnd,
		ArrowUp01,
		ArrowUp10,
		ArrowUpAZ,
		ArrowUpZA,
		Plus,
		Copy,
		Ambulance,
		Trash,
		X
	} from 'lucide-svelte';
	import { assetLoader, debounce } from '$utils';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { getAppState, getSocketState, getModalState, getNavigationState } from '$states';
	import { HealthBadge } from '$components';

	type SortBy = 'name' | 'level';
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
		palData: any;
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
	let sortBy: SortBy | undefined = $state(undefined);
	let sortOrder: SortOrder | undefined = $state(undefined);
	let alphaIcon: string = $state('');
	let foodIcon: string = $state('');
	let hpIcon: string = $state('');
	let containerRef: HTMLDivElement;
	let listWrapperStyle = $state('');
	let selectAll: boolean = $state(false);

	const sortByLevelAscClass = $derived(
		cn('btn', sortBy === 'level' && sortOrder === 'asc' ? 'bg-secondary-500/25' : '')
	);

	const sortByLevelDescClass = $derived(
		cn('btn', sortBy === 'level' && sortOrder === 'desc' ? 'bg-secondary-500/25' : '')
	);

	const sortByNameAscClass = $derived(
		cn('btn', sortBy === 'name' && sortOrder === 'asc' ? 'bg-secondary-500/25' : '')
	);

	const sortByNameDescClass = $derived(
		cn('btn', sortBy === 'name' && sortOrder === 'desc' ? 'bg-secondary-500/25' : '')
	);

	const sortLuckyClass = $derived(
		cn('btn', selectedFilter === 'lucky' ? 'bg-secondary-500/25' : '')
	);
	const sortAlphaClass = $derived(
		cn('btn', selectedFilter === 'alpha' ? 'bg-secondary-500/25' : '')
	);

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

	async function getPalsInfo() {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return;

		const playerPals = Object.entries(appState.selectedPlayer.pals as Record<string, Pal>);
		pals = await Promise.all(
			playerPals.map(async ([id, pal]) => {
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
					palData.type.map((e: string) => e.toLowerCase()).includes(selectedFilter.toLowerCase()));
			const matchesAlpha = selectedFilter === 'alpha' ? pal.is_boss : true;
			const matchesLucky = selectedFilter === 'lucky' ? pal.is_lucky : true;
			return matchesSearch && matchesElement && matchesAlpha && matchesLucky;
		});

		if (sortBy && sortOrder) {
			handleSort(sortBy, sortOrder);
		}
	}

	function handleSort(sortBy: SortBy, sortOrder: SortOrder | undefined) {
		switch (sortBy) {
			case 'name':
				sortByName(sortOrder || 'asc');
				break;
			case 'level':
				sortByLevel(sortOrder || 'asc');
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

	function getElementBadge(elementType: string): string | undefined {
		return elementBadges[elementType];
	}

	function getElementIcon(elementType: string): string | undefined {
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
		const message = {
			type: MessageType.ADD_PAL,
			data: {
				player_id: appState.selectedPlayer.uid,
				pal_code_name: selectedPal,
				nickname: nickname
			}
		};
		ws.send(JSON.stringify(message));
	}

	function sortByName(order: string) {
		sortBy = 'name';
		if (order === 'asc') {
			filteredPals = filteredPals.sort((a, b) => a.pal.name.localeCompare(b.pal.name));
			sortOrder = 'asc';
		} else if (order === 'desc') {
			filteredPals = filteredPals.sort((a, b) => b.pal.name.localeCompare(a.pal.name));
			sortOrder = 'desc';
		}
	}

	function sortByLevel(order: string) {
		sortBy = 'level';
		if (order === 'asc') {
			filteredPals = filteredPals.sort((a, b) => a.pal.level - b.pal.level);
			sortOrder = 'asc';
		} else if (order === 'desc') {
			filteredPals = filteredPals.sort((a, b) => b.pal.level - a.pal.level);
			sortOrder = 'desc';
		}
	}

	function getGenderIcon(gender: PalGender): string | undefined {
		return genderIcons[gender];
	}

	async function cloneSelectedPal(event: Event) {
		if (appState.selectedPlayer && appState.selectedPlayer.pals && selectedPal) {
			const pal = appState.selectedPlayer.pals[selectedPal.id];
			if (!pal) return;
			const message = {
				type: MessageType.CLONE_PAL,
				data: pal
			};
			ws.send(JSON.stringify(message));
		}
	}

	async function healSelectedPals() {
		if (appState.selectedPlayer && appState.selectedPlayer.pals) {
			const selectedPalIds = selectedPals.map((p) => p.pal.instance_id);
			const message = {
				type: MessageType.HEAL_PALS,
				data: selectedPalIds
			};
			ws.send(JSON.stringify(message));
			Object.values(appState.selectedPlayer.pals).forEach((pal) => {
				if (selectedPalIds.includes(pal.instance_id)) {
					pal.hp = pal.max_hp;
					pal.sanity = 100;
				}
			});
		}
		selectedPals = [];
		selectAll = false;
	}

	async function deleteSelectedPals() {
		if (selectedPals.length === 0) return;
		const confirmed = await modal.showConfirmModal({
			title: 'Delete Pal(s)',
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
			<Accordion.Item id="filter" controlHover="hover:bg-secondary-500/25">
				{#snippet controlLead()}<Search />{/snippet}
				{#snippet control()}
					<div class="flex flex-row items-center">
						<Search class="mr-2 h-5 w-5" />
						<span class="font-bold">Filter & Sort</span>
					</div>
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
						<div class="grid grid-cols-5">
							<Tooltip>
								<button
									type="button"
									class={sortByLevelAscClass}
									onclick={() => sortByLevel('asc')}
								>
									<ArrowUp01 />
								</button>
								{#snippet popup()}
									Sort by level in ascending order
								{/snippet}
							</Tooltip>
							<Tooltip>
								<button
									type="button"
									class={sortByLevelDescClass}
									onclick={() => sortByLevel('desc')}
								>
									<ArrowUp10 />
								</button>
								{#snippet popup()}
									Sort by level in descending order
								{/snippet}
							</Tooltip>
							<Tooltip>
								<button type="button" class={sortByNameAscClass} onclick={() => sortByName('asc')}>
									<ArrowUpAZ />
								</button>
								{#snippet popup()}
									Sort by name in ascending order
								{/snippet}
							</Tooltip>
							<Tooltip>
								<button
									type="button"
									class={sortByNameDescClass}
									onclick={() => sortByName('desc')}
								>
									<ArrowUpZA />
								</button>
								{#snippet popup()}
									Sort by name in descending order
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
							{#each p.palData.type as elementType}
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
			{/snippet}
			{#snippet listItemPopup(p)}
				<HealthBadge bind:pal={p.pal} player={appState.selectedPlayer} />
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
