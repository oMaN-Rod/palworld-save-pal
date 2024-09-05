<script lang="ts">
	import { cn } from '$theme';
	import { MessageType, PalGender, type Pal, type Player } from '$types';
	import { elementsData, palsData, getStats } from '$lib/data';
	import { Input, Tooltip, Checkbox, List } from '$components/ui';
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
	import { getAppState, getSocketState, getModalState } from '$states';
	import { HealthBadge } from '$components';

	type SortBy = 'name' | 'level';
	type SortOrder = 'asc' | 'desc';

	const appState = getAppState();
	const ws = getSocketState();
	const modal = getModalState();

	let { ...additionalProps } = $props<{
		[key: string]: any;
	}>();

	let searchQuery = $state('');
	let selectedFilter = $state('All');
	let elementTypes: string[] = $state([]);
	let elementIcons: Record<string, string> = $state({});
	let filteredPals: Pal[] = $state([]);
	let sortBy: SortBy | undefined = $state(undefined);
	let sortOrder: SortOrder | undefined = $state(undefined);
	let selectedPals: Pal[] = $state([]);
	let alphaIcon: string = $state('');
	let foodIcon: string = $state('');
	let hpIcon: string = $state('');

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

	async function filterPals() {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return;

		const pals = Object.entries(appState.selectedPlayer.pals as Record<string, Pal>);
		const palsWithInfo = await Promise.all(
			pals.map(async ([id, pal]) => {
				const palInfo = await palsData.getPalInfo(pal.character_id);
				await getStats(pal, appState.selectedPlayer as Player);
				return { id, pal, palInfo };
			})
		);

		selectedPals = [];
		filteredPals = palsWithInfo
			.filter(({ pal, palInfo }) => {
				const matchesSearch =
					pal.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
					pal.nickname?.toLowerCase().includes(searchQuery.toLowerCase());
				const matchesElement =
					selectedFilter === 'All' ||
					selectedFilter === 'alpha' ||
					selectedFilter === 'lucky' ||
					(palInfo &&
						palInfo.elements.map((e) => e.toLowerCase()).includes(selectedFilter.toLowerCase()));
				const matchesAlpha = selectedFilter === 'alpha' ? pal.is_boss : true;
				const matchesLucky = selectedFilter === 'lucky' ? pal.is_lucky : true;
				return matchesSearch && matchesElement && matchesAlpha && matchesLucky;
			})
			.map(({ id, pal }) => ({ id, ...pal }));

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

	function handlePalSelect(pal: Pal) {
		if (appState.selectedPlayer && appState.selectedPlayer.pals) {
			appState.selectedPal = appState.selectedPlayer.pals[pal.instance_id];
		}
	}

	async function loadStaticIcons() {
		const iconPath = `${ASSET_DATA_PATH}/img/icons/Alpha.png`;
		const icon = await assetLoader.loadImage(iconPath);
		alphaIcon = icon;

		const foodPath = `${ASSET_DATA_PATH}/img/icons/Food.png`;
		const food = await assetLoader.loadImage(foodPath);
		foodIcon = food;

		const hpPath = `${ASSET_DATA_PATH}/img/icons/Heart.png`;
		const hp = await assetLoader.loadImage(hpPath);
		hpIcon = hp;
	}

	async function loadElementTypes() {
		elementTypes = await elementsData.getAllElementTypes();
	}

	async function loadElementIcons() {
		for (const elementType of elementTypes) {
			const elementObj = await elementsData.searchElement(elementType);
			if (elementObj) {
				const iconPath = `${ASSET_DATA_PATH}/img/elements/${elementObj.badge_icon}.png`;
				try {
					elementIcons[elementType] = await assetLoader.loadImage(iconPath, true);
				} catch (error) {
					console.error(`Failed to load icon for ${elementType}:`, error);
				}
			}
		}
	}

	async function getPalElementBadge(elementType: string): Promise<string | undefined> {
		const elementObj = await elementsData.searchElement(elementType);
		if (!elementObj) return undefined;
		const icon_path = `${ASSET_DATA_PATH}/img/elements/${elementObj.badge_icon}.png`;
		const icon = await assetLoader.loadImage(icon_path, true);
		return icon;
	}

	async function getPalElementIcon(elementType: string): Promise<string | undefined> {
		const elementObj = await elementsData.searchElement(elementType);
		if (!elementObj) return undefined;
		const icon_path = `${ASSET_DATA_PATH}/img/elements/${elementObj.icon}.png`;
		const icon = await assetLoader.loadImage(icon_path, true);
		return icon;
	}

	async function getPalIcon(palId: string): Promise<string | undefined> {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return undefined;
		const pal = appState.selectedPlayer.pals[palId];
		if (!pal) return undefined;
		const palImgName = pal.name.toLowerCase().replaceAll(' ', '_');
		const icon_path = `${ASSET_DATA_PATH}/img/pals/menu/${palImgName}_menu.png`;
		const icon = await assetLoader.loadImage(icon_path, true);
		return icon;
	}

	async function handleAddPal() {
		if (!appState.selectedPlayer) return;
		// @ts-ignore
		const [selectedPal, nickname] = await modal.showModal(PalSelectModal, {
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
		if (selectedPal) {
			console.log('Selected Pal:', selectedPal);
		}
	}

	function sortByName(order: string) {
		sortBy = 'name';
		if (order === 'asc') {
			filteredPals = filteredPals.sort((a, b) => a.name.localeCompare(b.name));
			sortOrder = 'asc';
		} else if (order === 'desc') {
			filteredPals = filteredPals.sort((a, b) => b.name.localeCompare(a.name));
			sortOrder = 'desc';
		}
	}

	function sortByLevel(order: string) {
		sortBy = 'level';
		if (order === 'asc') {
			filteredPals = filteredPals.sort((a, b) => a.level - b.level);
			sortOrder = 'asc';
		} else if (order === 'desc') {
			filteredPals = filteredPals.sort((a, b) => b.level - a.level);
			sortOrder = 'desc';
		}
	}

	async function getGenderIcon(gender: PalGender): Promise<string | undefined> {
		if (gender == PalGender.UNKNOWN) return undefined;
		const iconPath = `${ASSET_DATA_PATH}/img/icons/${gender.toLowerCase()}.svg`;
		const icon = await assetLoader.loadSvg(iconPath);
		return icon;
	}

	async function cloneSelectedPal() {
		console.log('Cloning selected pal');
		if (appState.selectedPlayer && selectedPals.length === 1) {
			const selectedPal = selectedPals[0];
			const message = {
				type: MessageType.CLONE_PAL,
				data: selectedPal
			};
			ws.send(JSON.stringify(message));
		}
	}

	async function healSelectedPals() {
		console.log('Healing selected pal(s)');
		if (appState.selectedPlayer && appState.selectedPlayer.pals) {
			const selectedPalIds = selectedPals.map((pal) => pal.instance_id);
			const message = {
				type: MessageType.HEAL_PALS,
				data: selectedPalIds
			};
			ws.send(JSON.stringify(message));
			Object.values(appState.selectedPlayer.pals).forEach((pal) => {
				if (selectedPalIds.includes(pal.instance_id)) {
					pal.hp = pal.max_hp;
					pal.stomach = pal.max_stomach;
					pal.sanity = 100;
				}
			});
		}
	}

	async function deleteSelectedPals() {
		const numOfSelectedPals = selectedPals.length;
		if (numOfSelectedPals === 0) return;
		const confirmed = await modal.showConfirmModal({
			title: 'Delete Pal(s)',
			message: `Are you sure you want to delete the ${numOfSelectedPals} selected pal${numOfSelectedPals == 1 ? '' : 's'}?`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});
		if (appState.selectedPlayer && appState.selectedPlayer.pals && confirmed) {
			const selectedPalIds = selectedPals.map((pal) => pal.instance_id);
			const data = {
				player_id: appState.selectedPlayer.uid,
				pal_ids: selectedPalIds
			};
			const message = {
				type: MessageType.DELETE_PALS,
				data
			};
			ws.send(JSON.stringify(message));
			selectedPals = [];
			appState.selectedPlayer.pals = Object.fromEntries(
				Object.entries(appState.selectedPlayer.pals).filter(([id]) => !selectedPalIds.includes(id))
			);
		}
	}

	$effect(() => {
		loadElementTypes();
		loadStaticIcons();
	});

	$effect(() => {
		if (elementTypes.length > 0) {
			loadElementIcons();
		}
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
</script>

<div class="flex w-full flex-col space-y-2" {...additionalProps}>
	<div class="w-full">
		<div class="btn-group bg-surface-900 mb-2 items-center rounded p-1">
			<Tooltip position="right">
				<button class="btn hover:preset-tonal-secondary p-2" onclick={handleAddPal}>
					<Plus />
				</button>
				{#snippet popup()}
					Add a new pal to your Pal box
				{/snippet}
			</Tooltip>
			{#if Object.keys(selectedPals).length === 1}
				<Tooltip>
					<button class="btn hover:preset-tonal-secondary p-2" onclick={cloneSelectedPal}>
						<Copy />
					</button>
					{#snippet popup()}
						Clone selected pal
					{/snippet}
				</Tooltip>
			{/if}
			{#if Object.keys(selectedPals).length >= 1}
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
				{#snippet control()}Filter{/snippet}
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
									{#await getPalElementIcon(element) then icon}
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
	<List items={filteredPals} onselect={handlePalSelect} bind:selectedItems={selectedPals}>
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
		{#snippet listItem(pal)}
			<div class="grid w-full grid-cols-[55px_auto_1fr_auto] gap-2">
				<div>
					<span class="font-bold">Lvl {pal.level}</span>
				</div>
				<div class="relative justify-start">
					{#if pal.is_boss}
						{#if alphaIcon}
							<div class="absolute -left-2 -top-1 h-5 w-5">
								<enhanced:img src={alphaIcon} alt="Aplha" class="pal-element-badge"></enhanced:img>
							</div>
						{/if}
					{/if}
					{#if pal.is_lucky}
						<div class="absolute -left-2 -top-1 h-5 w-5">✨</div>
					{/if}
					{#await getPalIcon(pal.instance_id) then icon}
						{#if icon}
							<enhanced:img src={icon} alt={pal.name} class="h-8 w-8"></enhanced:img>
						{/if}
					{/await}
					{#await getGenderIcon(pal.gender) then icon}
						{#if icon}
							{@const color =
								pal.gender == PalGender.MALE ? 'text-primary-300' : 'text-tertiary-300'}
							<div class={cn('absolute -right-4 -top-1 h-5 w-5', color)}>
								{@html icon}
							</div>
						{/if}
					{/await}
				</div>
				<div class="ml-4 flex flex-row justify-start">
					<div>{pal.nickname || pal.name}</div>
				</div>
				<div class="flex justify-end">
					{#if pal.character_id && pal.elements}
						{#each pal.elements as elementType}
							{#await getPalElementBadge(elementType) then icon}
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
		{#snippet listItemPopup(pal)}
			<HealthBadge {pal} player={appState.selectedPlayer} />
		{/snippet}
	</List>
</div>

<style>
	.pal-element-badge {
		width: 24px;
		height: 24px;
	}
</style>
