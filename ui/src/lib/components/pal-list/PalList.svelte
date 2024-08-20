<script lang="ts">
	import { cn } from '$theme';
	import { MessageType, type PalSummary } from '$types';
	import { elementsData, palsData } from '$lib/data';
	import { Input, Tooltip } from '$components/ui';
	import { Accordion, Switch } from '@skeletonlabs/skeleton-svelte';
	import {
		Search,
		GalleryVerticalEnd,
		ArrowUp01,
		ArrowUp10,
		ArrowUpAZ,
		ArrowUpZA
	} from 'lucide-svelte';
	import { assetLoader, debounce } from '$utils';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { getAppState, getSocketState } from '$states';

	type SortBy = 'name' | 'level';
	type SortOrder = 'asc' | 'desc';

	const appState = getAppState();
	const ws = getSocketState();

	let { ...additionalProps } = $props<{
		[key: string]: any;
	}>();

	let searchQuery = $state('');
	let selectedElement = $state('All');
	let elementTypes: string[] = $state([]);
	let elementIcons: Record<string, string> = $state({});
	let filteredPals: Array<PalSummary & { id: string }> = $state([]);
	let sortBy: SortBy | undefined = $state(undefined);
	let sortOrder: SortOrder | undefined = $state(undefined);

	const listContainerClass = $derived(cn('h-[calc(100vh-200px)] overflow-hidden'));
	const listClass = $derived(
		cn('list w-full h-full border-surface-900 border divide-y divide-surface-900 overflow-y-auto')
	);

	const itemClass = $derived(cn('list-item p-2 flex items-center cursor-pointer'));

	async function filterPals() {
		if (!appState.selectedPlayer || !appState.selectedPlayer.pals) return;
		const pals = Object.entries(appState.selectedPlayer.pals as Record<string, PalSummary>);
		const palsWithInfo = await Promise.all(
			pals.map(async ([id, pal]) => {
				const palInfo = await palsData.getPalInfo(pal.character_id);
				return { id, pal, palInfo };
			})
		);

		filteredPals = palsWithInfo
			.filter(({ pal, palInfo }) => {
				const matchesSearch =
					pal.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
					pal.nickname.toLowerCase().includes(searchQuery.toLowerCase());
				const matchesElement =
					selectedElement === 'All' ||
					(palInfo &&
						palInfo.elements.map((e) => e.toLowerCase()).includes(selectedElement.toLowerCase()));
				return matchesSearch && matchesElement;
			})
			.map(({ id, pal }) => ({ id, ...pal }));

		if (sortBy && sortOrder) {
			if (sortBy === 'name') {
				sortByName(sortOrder);
			} else if (sortBy === 'level') {
				sortByLevel(sortOrder);
			}
		}
	}

	const debouncedFilterPals = debounce(filterPals, 300);

	function handlePalSelect(palId: string) {
		appState.selectedPalId = palId;
	}

	function handleKeyDown(event: KeyboardEvent, palId: string) {
		if (event.key === 'Enter' || event.key === ' ') {
			handlePalSelect(palId);
		}
	}

	async function loadElementTypes() {
		elementTypes = await elementsData.getAllElementTypes();
	}

	async function loadElementIcons() {
		for (const elementType of elementTypes) {
			const elementObj = await elementsData.searchElement(elementType);
			if (elementObj) {
				const iconPath = `${ASSET_DATA_PATH}/img/elements/${elementObj.badge_icon}.webp`;
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
		const icon_path = `${ASSET_DATA_PATH}/img/elements/${elementObj.badge_icon}.webp`;
		const icon = await assetLoader.loadImage(icon_path, true);
		return icon;
	}

	async function getPalElementIcon(elementType: string): Promise<string | undefined> {
		const elementObj = await elementsData.searchElement(elementType);
		if (!elementObj) return undefined;
		const icon_path = `${ASSET_DATA_PATH}/img/elements/${elementObj.icon}.webp`;
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

	$effect(() => {
		loadElementTypes();
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
		searchQuery = searchQuery.trim();
		selectedElement = selectedElement.trim();
		debouncedFilterPals();
	});

	$effect(() => {
		if (
			(appState.selectedPal && appState.selectedPal.level) ||
			(appState.selectedPal && appState.selectedPal.nickname)
		) {
			debouncedFilterPals();
		}
	});

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
</script>

<div class="flex w-full flex-col space-y-2" {...additionalProps}>
	<div class="w-full">
		<Accordion classes="bg-surface-900">
			<Accordion.Item id="filter">
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
						<div class="btn-group">
							<Tooltip>
								<button type="button" class="btn" onclick={() => sortByLevel('asc')}>
									<ArrowUp01 />
								</button>
								{#snippet popup()}
									Sort by level in ascending order
								{/snippet}
							</Tooltip>
							<Tooltip>
								<button type="button" class="btn" onclick={() => sortByLevel('desc')}>
									<ArrowUp10 />
								</button>
								{#snippet popup()}
									Sort by level in descending order
								{/snippet}
							</Tooltip>
							<Tooltip>
								<button type="button" class="btn" onclick={() => sortByName('asc')}>
									<ArrowUpAZ />
								</button>
								{#snippet popup()}
									Sort by name in ascending order
								{/snippet}
							</Tooltip>
							<Tooltip>
								<button type="button" class="btn" onclick={() => sortByName('desc')}>
									<ArrowUpZA />
								</button>
								{#snippet popup()}
									Sort by name in descending order
								{/snippet}
							</Tooltip>
						</div>
					</div>
					<div>
						<legend class="font-bold">Element</legend>
						<hr />
						<div class="mt-2 grid grid-cols-5">
							<Tooltip>
								<button class="btn flex" onclick={() => (selectedElement = 'All')}>
									<GalleryVerticalEnd />
								</button>
								{#snippet popup()}All pals{/snippet}
							</Tooltip>
							{#each [...elementTypes] as element}
								<Tooltip>
									{#await getPalElementIcon(element) then icon}
										{#if icon}
											<button class="btn flex" onclick={() => (selectedElement = element)}>
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
						</div>
					</div>
				{/snippet}
			</Accordion.Item>
		</Accordion>
	</div>
	<div class={listContainerClass}>
		<ul class={listClass}>
			{#each filteredPals as pal (pal.id)}
				<li
					class={cn(
						'hover:bg-secondary-500/25',
						itemClass,
						appState.selectedPalId === pal.id ? 'bg-secondary-500/25' : ''
					)}
				>
					<button
						class="flex w-full items-center text-left"
						onclick={() => handlePalSelect(pal.id)}
						onkeydown={(event) => handleKeyDown(event, pal.id)}
					>
						<div class="grid w-full grid-cols-[55px_auto_1fr_auto] gap-2">
							<div>
								<span class="font-bold">Lvl {pal.level}</span>
							</div>
							<div class="justify-start">
								{#await getPalIcon(pal.id) then icon}
									{#if icon}
										<!-- svelte-ignore element_invalid_self_closing_tag -->
										<enhanced:img src={icon} alt={pal.name} class="h-8 w-8" />
									{/if}
								{/await}
							</div>
							<div class="justify-start">
								<div>{pal.nickname || pal.name}</div>
							</div>
							<div class="flex justify-end">
								{#if pal.character_id && pal.elements}
									{#each pal.elements as elementType}
										{#await getPalElementBadge(elementType) then icon}
											{#if icon}
												<!-- svelte-ignore element_invalid_self_closing_tag -->
												<enhanced:img src={icon} alt={elementType} class="pal-element-badge" />
											{/if}
										{/await}
									{/each}
								{/if}
							</div>
						</div>
					</button>
				</li>
			{/each}
		</ul>
	</div>
</div>

<style>
	.pal-element-badge {
		width: 24px;
		height: 24px;
	}
</style>
