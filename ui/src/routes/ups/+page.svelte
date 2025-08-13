<script lang="ts">
	import { onMount } from 'svelte';
	import { Combobox, Input, TooltipButton } from '$components/ui';
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
		ExportPalModal
	} from '$components/modals';
	import { cn } from '$theme';
	import { getUpsState, getModalState, getAppState, getToastState } from '$states';
	import type {
		UPSSortBy,
		UPSSortOrder,
		ImportToUpsModalResults,
		AddToCollectionResult
	} from '$types';

	import UPSPalGrid from './components/UPSPalGrid.svelte';
	import UPSCollectionsPanel from './components/UPSCollectionsPanel.svelte';
	import UPSTagsPanel from './components/UPSTagsPanel.svelte';
	import UPSStatsPanel from './components/UPSStatsPanel.svelte';
	import UPSPalList from './components/UPSPalList.svelte';

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
	const characterOptions = $derived(['All', ...upsState.availableCharacterIds]);

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

	function handleCharacterFilter(characterId: string) {
		upsState.updateCharacterFilter(characterId);
		upsState.loadPals(true);
	}

	function selectAll() {
		upsState.selectAllPals();
	}

	function clearSelection() {
		upsState.clearSelection();
	}

	async function deleteSelected() {
		if (upsState.selectedPals.size === 0) return;

		const confirmed = await modal.showConfirmModal({
			title: 'Delete Selected Pals',
			message: `Are you sure you want to delete ${upsState.selectedPals.size} selected pal${upsState.selectedPals.size > 1 ? 's' : ''}? This action cannot be undone.`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});

		if (confirmed) {
			await upsState.deleteSelectedPals();
		}
	}

	async function handleImportFromSave() {
		if (!appState.saveFile) {
			toast.add('No save file loaded', 'Error', 'error');
			return;
		}

		// @ts-ignore
		const result = await modal.showModal<ImportToUpsModalResults[]>(ImportToUpsModal, {
			title: 'Import from Save File',
			message: 'Select the source and options for importing Pals to UPS.'
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

				toast.add(
					`Successfully started import from ${sourceType.toUpperCase()}`,
					'Success',
					'success'
				);
			} catch (error) {
				console.error('Import failed:', error);
				toast.add('Import failed. Please try again.', 'Error', 'error');
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
			title: `Edit Tags for ${selectedUpsPals.length} Pals`,
			pals: selectedUpsPals
		});

		if (result) {
			// Update tags for all selected pals
			for (const palId of selectedPalIds) {
				await upsState.updatePal(palId, { tags: result });
			}

			// Refresh data
			await upsState.loadPals();
			toast.add(`Updated tags for ${selectedPalIds.length} pals`, 'Success', 'success');
		}
	}

	async function handleBulkAddToCollection() {
		if (upsState.selectedPals.size === 0) return;

		const selectedPalIds = Array.from(upsState.selectedPals);
		const selectedUpsPals = upsState.pals.filter((pal) => selectedPalIds.includes(pal.id));

		// @ts-ignore
		const result = await modal.showModal<AddToCollectionResult>(AddToCollectionModal, {
			title: `Manage Collection for ${selectedUpsPals.length} Pals`,
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
				toast.add(`Removed ${selectedPalIds.length} pals from collections`, 'Success', 'success');
			} else {
				toast.add(`Moved ${selectedPalIds.length} pals to collection`, 'Success', 'success');
			}
		}
	}

	async function handleBulkExport() {
		if (upsState.selectedPals.size === 0) return;

		const selectedPalIds = Array.from(upsState.selectedPals);
		const selectedUpsPals = upsState.pals.filter((pal) => selectedPalIds.includes(pal.id));

		// @ts-ignore
		const result = await modal.showModal<{ target: string; playerId?: string }>(ExportPalModal, {
			title: `Export ${selectedUpsPals.length} Pals`,
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
					`Successfully exported ${successCount} of ${selectedPalIds.length} pals`,
					'Success',
					'success'
				);
			}

			if (errors.length > 0) {
				console.error('Export errors:', errors);
				toast.add(`Failed to export ${errors.length} pals`, 'Warning', 'warning');
			}
		}
	}

	$effect(() => {
		searchInput = upsState.filters.search;
	});

	onMount(async () => {
		await upsState.loadAll();
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
				<h1 class="text-xl font-semibold">Universal Pal Storage</h1>
				<p class="text-surface-600 dark:text-surface-400 text-sm">
					{upsState.pagination.totalCount} pals in storage
				</p>
			</div>
		</div>

		<!-- View Controls -->
		<div class="flex items-center gap-2">
			<!-- Import Button (when Pals exist) -->
			{#if upsState.pagination.totalCount > 0 && appState.saveFile}
				<TooltipButton
					onclick={handleImportFromSave}
					class="rounded-md bg-blue-500 p-2 text-white hover:bg-blue-600"
					popupLabel="Import from Save"
				>
					<Plus class="h-4 w-4" />
				</TooltipButton>

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
				popupLabel="Toggle Collections"
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
				popupLabel="Toggle Tags"
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
				popupLabel="Toggle Statistics"
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
				popupLabel="Grid View"
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
				popupLabel="List View"
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
				<div class="flex gap-2">
					<div class="relative flex-1">
						<Search
							class="text-surface-500 absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 focus:border-none"
						/>
						<Input
							bind:value={searchInput}
							oninput={handleSearchInput}
							placeholder="Search pals by name, character ID, or notes..."
							inputClass="pl-10"
						/>
					</div>
					{#if upsState.hasSelectedPals}
						<TooltipButton
							onclick={handleBulkEditTags}
							class="rounded-md bg-blue-500 p-2 text-white hover:bg-blue-600"
							popupLabel="Edit Tags"
						>
							<Tag class="h-4 w-4" />
						</TooltipButton>
						<TooltipButton
							onclick={handleBulkAddToCollection}
							class="rounded-md bg-green-500 p-2 text-white hover:bg-green-600"
							popupLabel="Add to Collection"
						>
							<Folder class="h-4 w-4" />
						</TooltipButton>
						<TooltipButton
							onclick={handleBulkExport}
							class="rounded-md bg-purple-500 p-2 text-white hover:bg-purple-600"
							popupLabel="Export Selected"
						>
							<Upload class="h-4 w-4" />
						</TooltipButton>
						<TooltipButton
							onclick={deleteSelected}
							class="rounded-md bg-red-500 p-2 text-white hover:bg-red-600"
							popupLabel="Delete Selected"
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
								<span>Filters & Sorting</span>
								{#if upsState.filters.search || upsState.filters.characterId !== 'All' || upsState.filters.collectionId || upsState.filters.tags.length > 0}
									<span class="bg-primary-500 rounded-full px-2 py-0.5 text-xs text-white"
										>Active</span
									>
								{/if}
							</div>
						{/snippet}
						{#snippet panel()}
							<div class="space-y-4 p-4">
								<!-- Character Filter -->
								<div class="grid grid-cols-[auto_1fr] items-center gap-2">
									<Combobox
										label="Character Type"
										placeholder="Select Character Type"
										options={characterOptions.map((o) => ({
											value: o,
											label: o
										}))}
										value={upsState.filters.characterId}
										onchange={(e: any) => handleCharacterFilter(e.detail.value)}
									/>
									<div>
										<span class="mb-2 block text-sm font-medium">Sort By</span>
										<div class="flex flex-wrap gap-2">
											{#each [{ key: 'created_at', label: 'Created' }, { key: 'updated_at', label: 'Modified' }, { key: 'character_id', label: 'Character' }, { key: 'nickname', label: 'Name' }, { key: 'level', label: 'Level' }, { key: 'transfer_count', label: 'Transfers' }, { key: 'clone_count', label: 'Clones' }] as sortOption}
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
								</div>

								<!-- Sort Controls -->

								<!-- Clear Filters -->
								{#if upsState.filters.search || upsState.filters.characterId !== 'All' || upsState.filters.collectionId || upsState.filters.tags.length > 0}
									<div class="border-surface-300 dark:border-surface-700 border-t pt-2">
										<button
											onclick={clearFilters}
											class="text-primary-600 hover:text-primary-700 flex items-center gap-1 text-sm"
										>
											<X class="h-3 w-3" />
											Clear All Filters
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
					class="bg-surface-100 dark:bg-surface-800 flex items-center justify-between px-4 py-2 text-sm"
				>
					<div class="flex items-center gap-4">
						<span>
							{upsState.selectedPals.size} of {upsState.pals.length} selected
						</span>
						<div class="flex items-center gap-2">
							<button onclick={selectAll} class="text-primary-600 hover:text-primary-700">
								Select All
							</button>
							{#if upsState.hasSelectedPals}
								<span class="text-surface-400">•</span>
								<button onclick={clearSelection} class="text-primary-600 hover:text-primary-700">
									Clear Selection
								</button>
							{/if}
						</div>
					</div>
					<div class="text-surface-600 dark:text-surface-400">
						Page {currentPage} of {totalPages}
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
							No Pals in Storage
						</h3>
						<p class="text-surface-500 mb-4 max-w-md">
							Import Pals from your save files or receive them from other players to get started.
						</p>
						{#if appState.saveFile}
							<button
								class="bg-primary-500 hover:bg-primary-600 flex items-center gap-2 rounded-md px-4 py-2 text-white"
								onclick={handleImportFromSave}
							>
								<Plus class="h-4 w-4" />
								Import from Save
							</button>
						{/if}
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
							Showing {(currentPage - 1) * upsState.pagination.limit + 1} to {Math.min(
								currentPage * upsState.pagination.limit,
								upsState.pagination.totalCount
							)} of {upsState.pagination.totalCount} pals
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
