<script lang="ts">
	import { Input, TooltipButton } from '$components/ui';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import {
		Search,
		ArrowDown01,
		ArrowDown10,
		ArrowDownAZ,
		ArrowDownZA,
		Trash,
		X,
		ArrowDownWideNarrow,
		User,
		Grid3x3,
		List,
		Folder,
		Tag,
		BarChart3,
		Plus,
		Filter,
		Database,
		Upload
	} from 'lucide-svelte';
	import {
		ImportToUpsModal,
		EditTagsModal,
		AddToCollectionModal,
		ExportPalModal,
		NukeUpsConfirmModal,
		PalSelectModal
	} from '$components/modals';
	import { cn } from '$theme';
	import {
		getUpsState,
		getModalState,
		getAppState,
		getToastState,
		getNavigationState
	} from '$states';
	import { elementsData, palsData } from '$lib/data';
	import { goto } from '$app/navigation';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { staticIcons } from '$types/icons';
	import {
		type UPSSortBy,
		type UPSSortOrder,
		type ImportToUpsModalResults,
		type AddToCollectionResult,
		EntryState,
		PalGender,
		type WorkSuitability
	} from '$types';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	import UPSPalGrid from './components/UPSPalGrid.svelte';
	import UPSCollectionsPanel from './components/UPSCollectionsPanel.svelte';
	import UPSTagsPanel from './components/UPSTagsPanel.svelte';
	import UPSStatsPanel from './components/UPSStatsPanel.svelte';
	import UPSPalList from './components/UPSPalList.svelte';
	import Nuke from '$components/ui/icons/Nuke.svelte';

	const VISIBLE_PAGE_BUBBLES = 16;

	const upsState = getUpsState();
	const modal = getModalState();
	const appState = getAppState();
	const toast = getToastState();

	let searchInput = $state('');
	let searchTimeout: ReturnType<typeof setTimeout> | undefined = undefined;

	function handleSearchInput() {
		if (searchTimeout) {
			clearTimeout(searchTimeout);
		}
		searchTimeout = setTimeout(() => {
			upsState.updateSearch(searchInput);
			upsState.loadPals(true);
		}, 300);
	}

	const totalPages = $derived(upsState.pagination.totalPages);
	const currentPage = $derived(upsState.pagination.page);
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

	// Element and Type filtering
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

	function handleSort(sortBy: UPSSortBy) {
		const newOrder: UPSSortOrder =
			upsState.filters.sortBy === sortBy && upsState.filters.sortOrder === 'asc' ? 'desc' : 'asc';

		upsState.updateSort(sortBy, newOrder);
		upsState.loadPals(true);
	}

	function getSortIcon(sortBy: UPSSortBy) {
		if (upsState.filters.sortBy !== sortBy) {
			return ArrowDownWideNarrow;
		}
		return upsState.filters.sortOrder === 'asc' ? ArrowDown01 : ArrowDown10;
	}

	function handlePageChange(page: number) {
		if (page >= 1 && page <= totalPages) {
			upsState.setPage(page);
			upsState.loadPals();
		}
	}

	function clearFilters() {
		searchInput = '';
		upsState.clearFilters();
		upsState.loadPals(true);
	}

	function handleElementTypeFilter(elementType: string) {
		const currentTypes = [...upsState.filters.elementTypes];
		if (currentTypes.includes(elementType)) {
			// Remove if already selected
			const newTypes = currentTypes.filter((t) => t !== elementType);
			upsState.updateElementTypesFilter(newTypes);
		} else {
			// Add if not selected
			currentTypes.push(elementType);
			upsState.updateElementTypesFilter(currentTypes);
		}
		upsState.loadPals(true);
	}

	function handlePalTypeFilter(palType: string) {
		const currentTypes = [...upsState.filters.palTypes];
		if (currentTypes.includes(palType)) {
			// Remove if already selected
			const newTypes = currentTypes.filter((t) => t !== palType);
			upsState.updatePalTypesFilter(newTypes);
		} else {
			// Add if not selected
			currentTypes.push(palType);
			upsState.updatePalTypesFilter(currentTypes);
		}
		upsState.loadPals(true);
	}

	function clearElementTypeFilters() {
		upsState.updateElementTypesFilter([]);
		upsState.loadPals(true);
	}

	function clearPalTypeFilters() {
		upsState.updatePalTypesFilter([]);
		upsState.loadPals(true);
	}

	// Helper functions for styling
	function getElementButtonClass(element: string) {
		return cn('btn', upsState.filters.elementTypes.includes(element) ? 'bg-secondary-500/25' : '');
	}

	function getPalTypeButtonClass(palType: string) {
		return cn('btn', upsState.filters.palTypes.includes(palType) ? 'bg-secondary-500/25' : '');
	}

	function selectAll() {
		upsState.selectAllPals();
	}

	async function selectAllFiltered() {
		await upsState.selectAllFilteredPals();
	}

	function clearSelection() {
		upsState.clearSelection();
	}

	// Helper to determine if filters are active
	function hasActiveFilters(): boolean {
		return (
			!!upsState.filters.search ||
			!!upsState.filters.collectionId ||
			upsState.filters.tags.length > 0 ||
			upsState.filters.elementTypes.length > 0 ||
			upsState.filters.palTypes.length > 0
		);
	}

	async function deleteSelected() {
		if (upsState.selectedPals.size === 0) return;

		const confirmed = await modal.showConfirmModal({
			title: m.delete_selected_entity({ entity: m.pal({ count: upsState.selectedPals.size }) }),
			message: m.delete_entity_confirm({ entity: m.pal({ count: upsState.selectedPals.size }) }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});

		if (confirmed) {
			await upsState.deleteSelectedPals();
		}
	}

	async function handleImportFromSave() {
		if (!appState.saveFile) {
			toast.add(m.no_save_loaded(), m.error(), 'error');
			return;
		}

		// @ts-ignore
		const result = await modal.showModal<ImportToUpsModalResults[]>(ImportToUpsModal, {
			title: m.save_file_to_ups(),
			message: m.import_to_ups_message({ pals: c.pals })
		});

		if (!result) return;

		for (const importData of result) {
			const { sourceType, sourceSlot, collectionId, tags, notes, palId, playerId } = importData;

			try {
				await upsState.importFromSave(
					sourceType,
					palId,
					sourceSlot,
					playerId,
					collectionId,
					tags.length > 0 ? tags : undefined,
					notes || undefined
				);

				await upsState.loadAll();
			} catch (error) {
				console.error('Import failed:', error);
				toast.add(m.import_failed(), m.error(), 'error');
			}
		}
	}

	// Bulk Actions
	async function handleBulkEditTags() {
		if (upsState.selectedPals.size === 0) return;

		const selectedPalIds = Array.from(upsState.selectedPals);
		const selectedUpsPals = upsState.pals.filter((pal) => selectedPalIds.includes(pal.id));

		// @ts-ignore
		const result = await modal.showModal<string[]>(EditTagsModal, {
			title: m.edit_tags_for_pals({
				pals: m.pal({ count: selectedUpsPals.length }),
				count: selectedUpsPals.length
			}),
			pals: selectedUpsPals
		});

		if (result) {
			// Update tags for all selected pals
			for (const palId of selectedPalIds) {
				await upsState.updatePal(palId, { tags: result });
			}

			// Refresh data
			await upsState.loadPals();
			toast.add(
				m.updated_tags_for_pals({
					pals: m.pal({ count: selectedUpsPals.length }),
					count: selectedPalIds.length
				}),
				m.success(),
				'success'
			);
		}
	}

	async function handleBulkAddToCollection() {
		if (upsState.selectedPals.size === 0) return;

		const selectedPalIds = Array.from(upsState.selectedPals);
		const selectedUpsPals = upsState.pals.filter((pal) => selectedPalIds.includes(pal.id));

		// @ts-ignore
		const result = await modal.showModal<AddToCollectionResult>(AddToCollectionModal, {
			title: m.manage_collection_for_pals({
				pals: m.pal({ count: selectedUpsPals.length }),
				count: selectedUpsPals.length
			}),
			pals: selectedUpsPals
		});

		if (result) {
			// Update collection for all selected pals
			const collectionId = result.removeFromCollection ? undefined : result.collectionId;

			for (const palId of selectedPalIds) {
				await upsState.updatePal(palId, { collection_id: collectionId });
			}

			// Refresh data
			await upsState.loadAll();

			if (result.removeFromCollection) {
				toast.add(
					m.removed_pals_from_collections({
						pals: m.pal({ count: selectedPalIds.length }),
						count: selectedPalIds.length
					}),
					m.success(),
					'success'
				);
			} else {
				toast.add(
					m.moved_pals_to_collection({
						pals: m.pal({ count: selectedPalIds.length }),
						count: selectedPalIds.length
					}),
					m.success(),
					'success'
				);
			}
		}
	}

	async function handleBulkExport() {
		if (upsState.selectedPals.size === 0) return;

		const selectedPalIds = Array.from(upsState.selectedPals);
		const selectedUpsPals = upsState.pals.filter((pal) => selectedPalIds.includes(pal.id));

		// @ts-ignore
		const result = await modal.showModal<{ target: string; playerId?: string }>(ExportPalModal, {
			title: m.export_pals({
				pals: m.pal({ count: selectedUpsPals.length }),
				count: selectedUpsPals.length
			}),
			pals: selectedUpsPals
		});

		if (result) {
			let successCount = 0;
			const errors = [];

			for (const palId of selectedPalIds) {
				const target = result.target as 'pal_box' | 'dps' | 'gps';
				try {
					await upsState.exportPal(palId, target, result.playerId);
					successCount++;
				} catch (error) {
					console.error(`Failed to export pal ${palId}:`, error);
					errors.push(`Pal ${palId}: ${error}`);
				}
			}

			if (successCount > 0) {
				toast.add(
					m.successfully_exported_pals({
						pals: m.pal({ count: selectedPalIds.length }),
						count: successCount,
						total: selectedPalIds.length
					}),
					m.success(),
					'success'
				);
			}

			if (errors.length > 0) {
				console.error('Export errors:', errors);
				toast.add(
					m.failed_export_pals({
						pals: m.pal({ count: selectedPalIds.length }),
						count: errors.length
					}),
					m.warning(),
					'warning'
				);
			}
		}
	}

	async function handleNukeUps() {
		try {
			// @ts-ignore
			const confirmed = await modal.showModal<boolean>(NukeUpsConfirmModal, {
				totalPals: upsState.pagination.totalCount
			});

			if (!confirmed) {
				return;
			}

			// Perform the nuke operation
			const result = await upsState.nukeAllPals();

			if (result.success) {
				if (result.deletedCount > 0) {
					toast.add(
						m.successfully_deleted_entity({
							pals: c.pals,
							count: result.deletedCount,
							entity: m.universal_pal_storage({ pal: c.pal })
						}),
						m.success(),
						'success'
					);
				} else {
					toast.add(m.ups_already_empty(), m.info(), 'info');
				}
			} else {
				toast.add(m.failed_nuke_ups(), m.error(), 'error');
			}
		} catch (error) {
			console.error('Error during nuke operation:', error);
			toast.add(m.error_nuke_operation(), m.error(), 'error');
		}
	}

	async function handleAddPal() {
		// @ts-ignore
		const result = await modal.showModal<[string, string] | undefined>(PalSelectModal, {
			title: m.add_new_pal_to_entity({
				entity: m.universal_pal_storage({ pal: c.pal })
			})
		});

		if (!result) return;

		const [selectedPal, nickname] = result;
		const palData = palsData.getByKey(selectedPal);

		if (!palData) {
			toast.add(m.failed_get_pal_data(), m.error(), 'error');
			return;
		}

		try {
			// Create character_key using the same logic as backend
			let character_key = selectedPal.toLowerCase();
			if (character_key.startsWith('boss_')) {
				character_key = character_key.slice(5);
			} else if (character_key.startsWith('predator_')) {
				character_key = character_key.slice(9);
			} else if (character_key.endsWith('_avatar')) {
				character_key = character_key.slice(0, -7);
			}

			// Create a new pal instance with proper default values
			const newPal = {
				instance_id: '00000000-0000-0000-0000-000000000000',
				character_id: selectedPal,
				character_key: character_key,
				nickname: nickname || palData.localized_name,
				name: palData.localized_name,
				level: 1,
				exp: 0,
				rank: 1,
				rank_hp: 0,
				rank_attack: 0,
				rank_defense: 0,
				rank_craftspeed: 0,
				talent_hp: 50,
				talent_shot: 50,
				talent_defense: 50,
				hp: 100,
				max_hp: 100,
				sanity: 100,
				stomach: 100,
				is_lucky: false,
				is_boss: false,
				is_predator: false,
				is_tower: false,
				is_sick: false,
				gender: PalGender.MALE,
				friendship_point: 0,
				owner_uid: '',
				storage_id: '00000000-0000-0000-0000-000000000000', // Required UUID field
				storage_slot: 0,
				group_id: null, // Optional UUID field
				learned_skills: [],
				active_skills: [],
				passive_skills: [],
				work_suitability: {} as Record<WorkSuitability, number>,
				elements: palData.element_types || [],
				state: EntryState.MODIFIED,
				__ups_source: true,
				__ups_new: true
			};

			appState.addNewUpspal(newPal);
		} catch (error) {
			console.error('Failed to create new pal:', error);
			toast.add(m.failed_create_pal(), m.error(), 'error');
		}
	}

	$effect(() => {
		searchInput = upsState.filters.search;
	});
</script>

<div class="ups-container flex h-full flex-col">
	<!-- Header -->
	<div
		class="border-surface-300 dark:border-surface-700 flex items-center justify-between border-b p-4"
	>
		<div class="flex items-center gap-3">
			<div class="bg-primary-500 flex h-8 w-8 items-center justify-center rounded-lg">
				<Database class="h-5 w-5 text-white" />
			</div>
			<div>
				<h1 class="text-xl font-semibold">
					{m.universal_pal_storage({ pal: c.pal })}
				</h1>
				<p class="text-surface-600 dark:text-surface-400 text-sm">
					{upsState.pagination.totalCount}
					{m.pals_in_storage({ pals: m.pal({ count: upsState.pagination.totalCount }) })}
				</p>
			</div>
		</div>

		<!-- View Controls -->
		<div class="flex items-center gap-2">
			<!-- Add Pal Button -->
			<TooltipButton
				onclick={handleAddPal}
				class="rounded-md bg-green-500 p-2 text-white hover:bg-green-600"
				popupLabel={m.add_new_pal({ pal: c.pal })}
			>
				<Plus class="h-4 w-4" />
			</TooltipButton>

			<!-- Import Button (when Pals exist and save file is loaded) -->
			{#if appState.saveFile}
				<TooltipButton
					onclick={handleImportFromSave}
					class="rounded-md bg-blue-500 p-2 text-white hover:bg-blue-600"
					popupLabel={m.import_from_save()}
				>
					<Upload class="h-4 w-4" />
				</TooltipButton>
			{/if}

			{#if upsState.pagination.totalCount > 0 || appState.saveFile}
				<div class="bg-surface-300 dark:bg-surface-700 h-6 w-px"></div>
			{/if}

			<!-- Panel Toggles -->
			<TooltipButton
				onclick={() => upsState.toggleCollectionsPanel()}
				class={cn(
					'rounded-md p-2',
					upsState.showCollectionsPanel
						? 'bg-primary-500 text-white'
						: 'hover:bg-surface-200 dark:hover:bg-surface-800'
				)}
				popupLabel={m.toggle_entity({ entity: m.collection({ count: 2 }) })}
			>
				<Folder class="h-4 w-4" />
			</TooltipButton>

			<TooltipButton
				onclick={() => upsState.toggleTagsPanel()}
				class={cn(
					'rounded-md p-2',
					upsState.showTagsPanel
						? 'bg-primary-500 text-white'
						: 'hover:bg-surface-200 dark:hover:bg-surface-800'
				)}
				popupLabel={m.toggle_entity({ entity: c.tags })}
			>
				<Tag class="h-4 w-4" />
			</TooltipButton>

			<TooltipButton
				onclick={() => upsState.toggleStatsPanel()}
				class={cn(
					'rounded-md p-2',
					upsState.showStatsPanel
						? 'bg-primary-500 text-white'
						: 'hover:bg-surface-200 dark:hover:bg-surface-800'
				)}
				popupLabel={m.toggle_entity({ entity: m.statistics() })}
			>
				<BarChart3 class="h-4 w-4" />
			</TooltipButton>

			<div class="bg-surface-300 dark:bg-surface-700 h-6 w-px"></div>

			<!-- View Mode Toggle -->
			<TooltipButton
				onclick={() => upsState.setViewMode('grid')}
				class={cn(
					'rounded-md p-2',
					upsState.viewMode === 'grid'
						? 'bg-primary-500 text-white'
						: 'hover:bg-surface-200 dark:hover:bg-surface-800'
				)}
				popupLabel={m.grid_view()}
			>
				<Grid3x3 class="h-4 w-4" />
			</TooltipButton>

			<TooltipButton
				onclick={() => upsState.setViewMode('list')}
				class={cn(
					'rounded-md p-2',
					upsState.viewMode === 'list'
						? 'bg-primary-500 text-white'
						: 'hover:bg-surface-200 dark:hover:bg-surface-800'
				)}
				popupLabel={m.list_view()}
			>
				<List class="h-4 w-4" />
			</TooltipButton>
		</div>
	</div>

	<!-- Main Content -->
	<div class="flex flex-1 overflow-hidden">
		<!-- Side Panels -->
		{#if upsState.showCollectionsPanel || upsState.showTagsPanel || upsState.showStatsPanel}
			<div
				class="border-surface-300 dark:border-surface-700 bg-surface-50 dark:bg-surface-900 w-80 border-r"
			>
				{#if upsState.showCollectionsPanel}
					<UPSCollectionsPanel />
				{/if}
				{#if upsState.showTagsPanel}
					<UPSTagsPanel />
				{/if}
				{#if upsState.showStatsPanel}
					<UPSStatsPanel />
				{/if}
			</div>
		{/if}

		<!-- Main Panel -->
		<div class="flex flex-1 flex-col">
			<!-- Filters and Search -->
			<div class="border-surface-700 space-y-3 border-b p-4">
				<!-- Search Bar -->
				<div class="flex items-center gap-2">
					<div class="relative flex-1">
						<Search
							class="text-surface-500 absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 focus:border-none"
						/>
						<Input
							bind:value={searchInput}
							oninput={handleSearchInput}
							placeholder={m.search_pals({ pals: c.pals })}
							inputClass="pl-10"
						/>
					</div>
					{#if upsState.pagination.totalCount > 0}
						<TooltipButton popupLabel={m.nuke_ups({ pals: c.pals })}>
							<button
								class="text-error-500 hover:bg-error-500/20 hover:text-error-600 h-8 w-8 rounded-md p-2 transition-colors"
								onclick={handleNukeUps}
								disabled={upsState.loading}
							>
								<Nuke size={16} />
							</button>
						</TooltipButton>
					{/if}
					{#if upsState.hasSelectedPals}
						<TooltipButton
							onclick={handleBulkEditTags}
							class="rounded-md bg-blue-500 p-2 text-white hover:bg-blue-600"
							popupLabel={m.edit_entity({ entity: c.tags })}
						>
							<Tag class="h-4 w-4" />
						</TooltipButton>
						<TooltipButton
							onclick={handleBulkAddToCollection}
							class="rounded-md bg-green-500 p-2 text-white hover:bg-green-600"
							popupLabel={m.add_to_collection()}
						>
							<Folder class="h-4 w-4" />
						</TooltipButton>
						<TooltipButton
							onclick={handleBulkExport}
							class="rounded-md bg-purple-500 p-2 text-white hover:bg-purple-600"
							popupLabel={m.export_selected()}
						>
							<Upload class="h-4 w-4" />
						</TooltipButton>
						<TooltipButton
							onclick={deleteSelected}
							class="rounded-md bg-red-500 p-2 text-white hover:bg-red-600"
							popupLabel={m.delete_entity({ entity: m.selected() })}
						>
							<Trash class="h-4 w-4" />
						</TooltipButton>
					{/if}
				</div>

				<!-- Filter Controls -->
				<Accordion base="w-full" collapsible>
					<Accordion.Item value="filters">
						{#snippet control()}
							<div class="flex items-center gap-2">
								<Filter class="h-4 w-4" />
								<span>{m.filter_and_sort()}</span>
								{#if upsState.filters.search || upsState.filters.collectionId || upsState.filters.tags.length > 0 || upsState.filters.elementTypes.length > 0 || upsState.filters.palTypes.length > 0}
									<span class="bg-primary-500 rounded-full px-2 py-0.5 text-xs text-white"
										>{m.active()}</span
									>
								{/if}
							</div>
						{/snippet}
						{#snippet panel()}
							<div class="grid grid-cols-2 gap-4 p-4">
								<!-- Left Column: Elements & Types Filter -->
								<div class="space-y-4">
									<span class="block text-sm font-medium">{m.element_and_type()}</span>
									<div class="space-y-3">
										<!-- Element Types -->
										<div>
											<div class="mb-1 flex items-center justify-between">
												<span class="text-surface-600 dark:text-surface-400 text-xs font-medium"
													>{m.element_types()}</span
												>
												{#if upsState.filters.elementTypes.length > 0}
													<button
														class="text-primary-600 hover:text-primary-700 text-xs"
														onclick={clearElementTypeFilters}
													>
														{m.clear()} ({upsState.filters.elementTypes.length})
													</button>
												{/if}
											</div>
											<div class="grid grid-cols-6 gap-1">
												{#each elementTypes as element}
													{@const elementData = elementsData.getByKey(element)}
													{@const localizedName = elementData?.localized_name || element}
													<TooltipButton popupLabel={localizedName}>
														<button
															class={getElementButtonClass(element)}
															onclick={() => handleElementTypeFilter(element)}
															aria-label={localizedName}
														>
															<img
																src={elementIcons[element]}
																alt={localizedName}
																class="h-6 w-6"
															/>
														</button>
													</TooltipButton>
												{/each}
											</div>
										</div>

										<!-- Pal Types -->
										<div>
											<div class="mb-1 flex items-center justify-between">
												<span class="text-surface-600 dark:text-surface-400 text-xs font-medium"
													>{m.pal_types()}</span
												>
												{#if upsState.filters.palTypes.length > 0}
													<button
														class="text-primary-600 hover:text-primary-700 text-xs"
														onclick={clearPalTypeFilters}
													>
														{m.clear()} ({upsState.filters.palTypes.length})
													</button>
												{/if}
											</div>
											<div class="grid grid-cols-6 gap-1">
												<TooltipButton popupLabel={m.alpha_pal({ pals: c.pals })}>
													<button
														class={getPalTypeButtonClass('alpha')}
														onclick={() => handlePalTypeFilter('alpha')}
													>
														<img src={staticIcons.alphaIcon} alt="Alpha" class="h-6 w-6" />
													</button>
												</TooltipButton>
												<TooltipButton popupLabel={m.lucky_pals({ pals: c.pals })}>
													<button
														class={getPalTypeButtonClass('lucky')}
														onclick={() => handlePalTypeFilter('lucky')}
													>
														<img src={staticIcons.luckyIcon} alt="Lucky" class="h-6 w-6" />
													</button>
												</TooltipButton>
												<TooltipButton popupLabel={m.human({ count: 2 })}>
													<button
														class={getPalTypeButtonClass('human')}
														onclick={() => handlePalTypeFilter('human')}
													>
														<User class="h-6 w-6" />
													</button>
												</TooltipButton>
												<TooltipButton popupLabel={m.predator_pals({ pals: c.pals })}>
													<button
														class={getPalTypeButtonClass('predator')}
														onclick={() => handlePalTypeFilter('predator')}
													>
														<img src={staticIcons.predatorIcon} alt="Predator" class="h-6 w-6" />
													</button>
												</TooltipButton>
												<TooltipButton popupLabel={m.oil_rig_pals({ pals: c.pals })}>
													<button
														class={getPalTypeButtonClass('oilrig')}
														onclick={() => handlePalTypeFilter('oilrig')}
													>
														<img src={staticIcons.oilrigIcon} alt="Oil Rig" class="h-6 w-6" />
													</button>
												</TooltipButton>
												<TooltipButton popupLabel={m.summoned_pals({ pals: c.pals })}>
													<button
														class={getPalTypeButtonClass('summon')}
														onclick={() => handlePalTypeFilter('summon')}
													>
														<img src={staticIcons.altarIcon} alt="Summoned" class="h-6 w-6" />
													</button>
												</TooltipButton>
											</div>
										</div>
									</div>
								</div>

								<!-- Right Column: Sort By -->
								<div>
									<span class="mb-2 block text-sm font-medium">{m.sort_by()}</span>
									<div class="flex flex-wrap gap-2">
										{#each [{ key: 'created_at', label: m.created() }, { key: 'updated_at', label: m.modified() }, { key: 'character_id', label: m.character() }, { key: 'nickname', label: m.name() }, { key: 'level', label: m.level() }, { key: 'transfer_count', label: m.transfer( { count: 2 } ) }, { key: 'clone_count', label: m.clones() }] as sortOption}
											{@const IconComponent = getSortIcon(sortOption.key as UPSSortBy)}
											<button
												class={cn(
													'flex items-center gap-1 rounded-md border px-3 py-1 text-sm',
													upsState.filters.sortBy === sortOption.key
														? 'bg-primary-500 border-primary-500 text-white'
														: 'dark:bg-surface-800 border-surface-300 dark:border-surface-700 bg-white'
												)}
												onclick={() => handleSort(sortOption.key as UPSSortBy)}
											>
												{sortOption.label}
												<IconComponent class="h-3 w-3" />
											</button>
										{/each}
									</div>
								</div>

								<!-- Clear Filters (spans both columns) -->
								{#if upsState.filters.search || upsState.filters.collectionId || upsState.filters.tags.length > 0 || upsState.filters.elementTypes.length > 0 || upsState.filters.palTypes.length > 0}
									<div class="border-surface-300 dark:border-surface-700 col-span-2 border-t pt-2">
										<button
											onclick={clearFilters}
											class="text-primary-600 hover:text-primary-700 flex items-center gap-1 text-sm"
										>
											<X class="h-3 w-3" />
											{m.clear_all_entity({ entity: m.filter({ count: 2 }) })}
										</button>
									</div>
								{/if}
							</div>
						{/snippet}
					</Accordion.Item>
				</Accordion>
			</div>

			<!-- Selection Controls -->
			{#if upsState.pals.length > 0}
				<div
					class="bg-surface-100 dark:bg-surface-800 flex items-center justify-between px-4 text-sm"
				>
					<div class="flex items-center gap-4">
						<span>
							{m.selected_of_total({
								selected: upsState.selectedPals.size,
								total:
									upsState.selectedPals.size <= upsState.pals.length
										? upsState.pals.length
										: upsState.pagination.totalCount
							})}
						</span>
						<div class="flex items-center">
							<nav class="btn-group flex-row p-0">
								<TooltipButton
									popupLabel={m.select_all_page_pals({
										pals: c.pals,
										count: upsState.pals.length
									})}
								>
									<button
										type="button"
										class="btn hover:preset-tonal px-2 text-xs"
										onclick={selectAll}
									>
										{m.page()}
									</button>
								</TooltipButton>
								{#if hasActiveFilters()}
									<TooltipButton
										popupLabel={m.select_all_filtered_pals({
											pals: c.pals,
											count: upsState.pagination.totalCount
										})}
									>
										<button
											type="button"
											class="btn hover:preset-tonal px-2 text-xs"
											onclick={selectAllFiltered}
										>
											{m.filtered()}
										</button>
									</TooltipButton>
								{:else}
									<TooltipButton
										popupLabel={m.select_all_ups_pals({
											pals: c.pals,
											count: upsState.pagination.totalCount
										})}
									>
										<button
											type="button"
											class="btn hover:preset-tonal px-2 text-xs"
											onclick={selectAllFiltered}
										>
											{m.all_entity({ entity: m.ups() })}
										</button>
									</TooltipButton>
								{/if}
							</nav>
							{#if upsState.hasSelectedPals}
								<span class="text-surface-400">•</span>
								<button onclick={clearSelection} class="text-primary-600 hover:text-primary-700">
									{m.clear_selection()}
								</button>
							{/if}
						</div>
					</div>
					<div class="text-surface-600 dark:text-surface-400">
						{m.page_of_pages({ current: currentPage, total: totalPages })}
					</div>
				</div>
			{/if}

			<!-- Pals Content -->
			<div class="flex-1 overflow-auto">
				{#if upsState.loading}
					<div class="flex h-32 items-center justify-center">
						<div class="border-primary-500 h-8 w-8 animate-spin rounded-full border-b-2"></div>
					</div>
				{:else if upsState.pals.length === 0}
					<div class="flex h-64 flex-col items-center justify-center text-center">
						<User class="text-surface-400 mb-4 h-16 w-16" />
						<h3 class="text-surface-600 dark:text-surface-400 mb-2 text-lg font-medium">
							{m.no_pals_in_storage({ pals: c.pals })}
						</h3>
						<p class="text-surface-500 mb-4 max-w-md">
							{m.create_pals_or_import({ pals: c.pals })}
						</p>
						<div class="flex gap-3">
							<button
								class="flex items-center gap-2 rounded-md bg-green-500 px-4 py-2 text-white hover:bg-green-600"
								onclick={handleAddPal}
							>
								<Plus class="h-4 w-4" />
								{m.add_new_pal({ pal: c.pal })}
							</button>
							{#if appState.saveFile}
								<button
									class="flex items-center gap-2 rounded-md bg-blue-500 px-4 py-2 text-white hover:bg-blue-600"
									onclick={handleImportFromSave}
								>
									<Upload class="h-4 w-4" />
									{m.import_from_save()}
								</button>
							{/if}
						</div>
					</div>
				{:else}
					<!-- Pal Grid/List -->
					{#if upsState.viewMode === 'grid'}
						<UPSPalGrid />
					{:else}
						<UPSPalList />
					{/if}
				{/if}
			</div>

			<!-- Pagination -->
			{#if totalPages > 1}
				<div class="border-surface-300 dark:border-surface-700 border-t p-4">
					<div class="flex items-center justify-between">
						<div class="text-surface-600 dark:text-surface-400 text-sm">
							{m.showing_pals({
								start: (currentPage - 1) * upsState.pagination.limit + 1,
								end: Math.min(
									currentPage * upsState.pagination.limit,
									upsState.pagination.totalCount
								),
								total: upsState.pagination.totalCount,
								pals: c.pals
							})}
						</div>
						<div class="flex items-center gap-1">
							<button
								onclick={() => handlePageChange(1)}
								disabled={currentPage === 1}
								class="hover:bg-surface-200 dark:hover:bg-surface-800 rounded p-2 disabled:cursor-not-allowed disabled:opacity-50"
							>
								<ArrowDown01 class="h-4 w-4 rotate-90" />
							</button>
							<button
								onclick={() => handlePageChange(currentPage - 1)}
								disabled={currentPage === 1}
								class="hover:bg-surface-200 dark:hover:bg-surface-800 rounded p-2 disabled:cursor-not-allowed disabled:opacity-50"
							>
								<ArrowDownAZ class="h-4 w-4 rotate-90" />
							</button>

							{#each visiblePages as page}
								<button
									onclick={() => handlePageChange(page)}
									class={cn(
										'rounded px-3 py-1 text-sm',
										page === currentPage
											? 'bg-primary-500 text-white'
											: 'hover:bg-surface-200 dark:hover:bg-surface-800'
									)}
								>
									{page}
								</button>
							{/each}

							<button
								onclick={() => handlePageChange(currentPage + 1)}
								disabled={currentPage === totalPages}
								class="hover:bg-surface-200 dark:hover:bg-surface-800 rounded p-2 disabled:cursor-not-allowed disabled:opacity-50"
							>
								<ArrowDownZA class="h-4 w-4 -rotate-90" />
							</button>
							<button
								onclick={() => handlePageChange(totalPages)}
								disabled={currentPage === totalPages}
								class="hover:bg-surface-200 dark:hover:bg-surface-800 rounded p-2 disabled:cursor-not-allowed disabled:opacity-50"
							>
								<ArrowDown10 class="h-4 w-4 -rotate-90" />
							</button>
						</div>
					</div>
				</div>
			{/if}
		</div>
	</div>
</div>

<style>
	.ups-container {
		background: var(--color-surface-50);
	}

	:global(.dark) .ups-container {
		background: var(--color-surface-950);
	}
</style>
