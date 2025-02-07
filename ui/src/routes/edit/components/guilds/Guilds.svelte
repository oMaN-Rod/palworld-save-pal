<script lang="ts">
	import { palsData } from '$lib/data';
	import { getAppState, getSocketState, getModalState, getToastState } from '$states';
	import { Tooltip, TooltipButton } from '$components/ui';
	import { type Pal, MessageType } from '$types';
	import { staticIcons } from '$lib/constants';
	import { Ambulance, X, ReplaceAll, Plus, Trash, Bandage } from 'lucide-svelte';
	import { PalBadge } from '$components';
	import { PalSelectModal, NumberInputModal } from '$components/modals';
	import { debounce, deepCopy, formatNickname } from '$utils';

	interface PalWithBaseId {
		pal: Pal;
		baseId: string;
	}

	const appState = getAppState();
	const ws = getSocketState();
	const modal = getModalState();
	const toast = getToastState();

	const VISIBLE_PAGE_BUBBLES = 16;

	let selectedPals: string[] = $state([]);
	let searchQuery = $state('');
	let currentPage = $state(1);
	let filteredPals: PalWithBaseId[] = $state([]);

	const playerGuild = $derived.by(() => {
		if (appState.selectedPlayer?.guild_id) {
			return appState.guilds[appState.selectedPlayer.guild_id];
		}
	});
	$inspect(playerGuild);

	const guildBases = $derived.by(() => {
		if (playerGuild) {
			return playerGuild.bases;
		}
	});

	const totalPages = $derived(Object.keys(guildBases || {}).length);

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

	const currentBase = $derived.by(() => {
		if (!guildBases) return null;
		const baseEntries = Object.entries(guildBases);
		return baseEntries[currentPage - 1] || null;
	});

	const currentPageItems = $derived.by(() => {
		if (!currentBase) return [];
		const [baseId, base] = currentBase;

		if (searchQuery) {
			return filteredPals;
		}

		return Array(base.slot_count)
			.fill(undefined)
			.map((_, index) => {
				const existingPal = Object.values(base.pals).find((p) => p.storage_slot === index);
				if (existingPal) {
					return {
						pal: existingPal,
						baseId: baseId
					};
				}
				return {
					pal: {
						character_id: 'None',
						character_key: 'None',
						storage_slot: index,
						instance_id: `empty-${index}`,
						storage_id: base.container_id
					},
					baseId: baseId
				} as PalWithBaseId;
			});
	});

	const debouncedFilterPals = debounce(filterPals, 300);

	function handleKeydown(event: KeyboardEvent) {
		if (event.target instanceof HTMLInputElement) return;

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

	function handlePalSelect(pal: Pal, event: MouseEvent) {
		if (!pal || pal.character_id === 'None') return;
		if (event.ctrlKey || event.metaKey) {
			if (selectedPals.includes(pal.instance_id)) {
				selectedPals = selectedPals.filter((id) => id !== pal.instance_id);
			} else {
				selectedPals = [...selectedPals, pal.instance_id];
			}
		}
	}

	async function handleAddPal(baseId: string, index?: number) {
		if (!appState.selectedPlayer || !guildBases) return;
		const base = guildBases[baseId];
		if (!base) return;

		// @ts-ignore
		const result = await modal.showModal<[string, string] | undefined>(PalSelectModal, {
			title: 'Add a new Pal'
		});
		if (!result) return;

		const [selectedPal, nickname] = result;
		const palData = palsData.pals[selectedPal];

		const message = {
			type: MessageType.ADD_PAL,
			data: {
				guild_id: playerGuild?.id,
				base_id: baseId,
				character_id: selectedPal,
				nickname: nickname || formatNickname(palData?.localized_name || selectedPal),
				container_id: base.container_id,
				storage_slot: index
			}
		};
		ws.send(JSON.stringify(message));
	}

	async function handleClonePal(item: PalWithBaseId) {
		if (!guildBases) return;
		const base = guildBases[item.baseId];
		if (!base) return;

		const maxClones = base.slot_count - Object.keys(base.pals).length;
		if (maxClones === 0) {
			toast.add('There are no slots available in this base.', 'Error', 'error');
			return;
		}

		// @ts-ignore
		const result = await modal.showModal<number>(NumberInputModal, {
			title: 'How many clones?',
			message: `There are ${maxClones} slots available in this base.`,
			value: 1,
			min: 0,
			max: maxClones
		});
		if (!result) return;

		for (let i = 0; i < result; i++) {
			const clonedPal = deepCopy(item.pal);
			clonedPal.nickname = formatNickname(
				clonedPal.nickname || clonedPal.name || clonedPal.character_id,
				'clone'
			);
			const message = {
				type: MessageType.CLONE_PAL,
				data: {
					guild_id: playerGuild!.id,
					base_id: item.baseId,
					pal: clonedPal
				}
			};
			ws.send(JSON.stringify(message));
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

		if (confirmed) {
			// get base id based on current page
			const baseId = currentBase ? currentBase[0] : '';
			const message = {
				type: MessageType.DELETE_PALS,
				data: {
					guild_id: playerGuild?.id,
					base_id: baseId,
					pal_ids: selectedPals
				}
			};
			ws.send(JSON.stringify(message));

			playerGuild!.bases[baseId].pals = Object.fromEntries(
				Object.entries(playerGuild!.bases[baseId].pals).filter(([id]) => !selectedPals.includes(id))
			);
		}

		selectedPals = [];
	}

	async function handleDeletePal(baseId: string, pal: Pal) {
		const confirmed = await modal.showConfirmModal({
			title: 'Delete Pal',
			message: `Are you sure you want to delete ${pal.nickname || pal.name}?`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});

		if (appState.selectedPlayer && confirmed) {
			const message = {
				type: MessageType.DELETE_PALS,
				data: {
					guild_id: playerGuild?.id,
					base_id: baseId,
					pal_ids: [pal.instance_id]
				}
			};
			ws.send(JSON.stringify(message));
		}
		playerGuild!.bases[baseId].pals = Object.fromEntries(
			Object.entries(playerGuild!.bases[baseId].pals).filter(
				([_, p]) => p.instance_id !== pal.instance_id
			)
		);
	}

	function filterPals() {
		if (!guildBases || !searchQuery) return;

		filteredPals = Object.entries(guildBases).flatMap(([baseId, base]) =>
			Object.values(base.pals)
				.filter((pal) => {
					return (
						pal.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
						pal.nickname?.toLowerCase().includes(searchQuery.toLowerCase()) ||
						pal.character_id.toLowerCase().includes(searchQuery.toLowerCase())
					);
				})
				.map((pal) => ({
					pal: pal,
					baseId: baseId
				}))
		);
	}

	function handleSelectAll() {
		if (!currentBase) return;
		const [_, base] = currentBase;

		const basePalIds = Object.values(base.pals).map((pal) => pal.instance_id);

		if (selectedPals.length === basePalIds.length) {
			selectedPals = [];
		} else {
			selectedPals = [...basePalIds];
		}
	}

	async function healSelectedPals() {
		if (!guildBases || selectedPals.length === 0) return;

		const message = {
			type: MessageType.HEAL_PALS,
			data: [...selectedPals]
		};
		ws.send(JSON.stringify(message));

		Object.values(guildBases).forEach((base) => {
			Object.values(base.pals).forEach((pal) => {
				if (selectedPals.includes(pal.instance_id)) {
					pal.hp = pal.max_hp;
					pal.sanity = 100;
					const palData = palsData.pals[pal.character_key];
					if (palData) {
						pal.stomach = palData.max_full_stomach;
					}
				}
			});
		});

		selectedPals = [];
	}

	$effect(() => {
		if (searchQuery) {
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
		if (currentPage > totalPages && totalPages > 0) {
			currentPage = totalPages;
		}
	});

	function handleHealAll() {
		if (!guildBases || !playerGuild || !currentBase) return;
		const message = {
			type: MessageType.HEAL_ALL_PALS,
			data: {
				guild_id: playerGuild.id,
				base_id: currentBase[0]
			}
		};
		ws.send(JSON.stringify(message));
		Object.values(guildBases).forEach((base) => {
			Object.values(base.pals).forEach((pal) => {
				pal.hp = pal.max_hp;
				pal.sanity = 100;
				pal.is_sick = false;
				const palData = palsData.pals[pal.character_key];
				if (palData) {
					pal.stomach = palData.max_full_stomach;
				}
			});
		});
	}
</script>

{#if appState.selectedPlayer}
	{#if !playerGuild}
		<div class="flex w-full items-center justify-center">
			<h2 class="h2">No Guild found</h2>
		</div>
	{:else if !guildBases}
		<div class="flex w-full items-center justify-center">
			<h2 class="h2">No Guild Bases found</h2>
		</div>
	{:else}
		<div class="grid h-full w-full grid-cols-[25%_1fr]">
			<!-- Left Controls -->
			<div class="mb-2 flex-shrink-0 p-4">
				<h4 class="h4">Base {currentPage}</h4>
				<div class="btn-group bg-surface-900 mb-2 items-center rounded p-1">
					<Tooltip position="right" label="Add Pal to Base">
						<button
							class="btn hover:preset-tonal-secondary p-2"
							onclick={() => currentBase && handleAddPal(currentBase[0])}
						>
							<Plus />
						</button>
					</Tooltip>
					<Tooltip label="Select all in current base">
						<button class="btn hover:preset-tonal-secondary p-2" onclick={handleSelectAll}>
							<ReplaceAll />
						</button>
					</Tooltip>
					<Tooltip label="Heal all in current base">
						<button class="btn hover:preset-tonal-secondary p-2" onclick={handleHealAll}>
							<Bandage />
						</button>
					</Tooltip>
					{#if selectedPals.length > 0}
						<Tooltip label="Heal selected pal(s)">
							<button class="btn hover:preset-tonal-secondary p-2" onclick={healSelectedPals}>
								<Ambulance />
							</button>
						</Tooltip>
						<Tooltip label="Delete selected pal(s)">
							<button class="btn hover:preset-tonal-secondary p-2" onclick={deleteSelectedPals}>
								<Trash />
							</button>
						</Tooltip>
						<Tooltip label="Clear selected">
							<button
								class="btn hover:preset-tonal-secondary p-2"
								onclick={() => (selectedPals = [])}
							>
								<X />
							</button>
						</Tooltip>
					{/if}
				</div>
			</div>

			<!-- Right Content -->
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
								popupLabel={`Base ${Object.entries(guildBases)[page - 1]?.[0]}`}
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
							{#if item.pal.character_id !== 'None' || !searchQuery}
								<PalBadge
									bind:pal={item.pal}
									bind:selected={selectedPals}
									onSelect={handlePalSelect}
									onDelete={() => handleDeletePal(currentBase![0], item.pal)}
									onAdd={() => handleAddPal(currentBase![0], item.pal.storage_slot)}
									onClone={() => handleClonePal(item)}
									onMove={() => {}}
								/>
							{/if}
						{/each}
					</div>
				</div>
			</div>
		</div>
	{/if}
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">Select a Player to view Guilds</h2>
	</div>
{/if}
