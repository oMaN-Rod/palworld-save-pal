<script lang="ts">
	import { untrack } from 'svelte';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';

	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Input, Nuke, Spinner, Tooltip, TooltipButton } from '$components/ui';
	import { ActionGroup, type ActionDescriptor } from '$components/ui/actions';
	import {
		ImportToUpsModal,
		EditTagsModal,
		AddToCollectionModal,
		ExportPalModal,
		NukeUpsConfirmModal,
		PalSelectModal
	} from '$components/modals';
	import { PalContainerView } from '$components/pal/container';
	import DetailPresentation from '$components/layout/DetailPresentation.svelte';
	import { layout } from '$utils/layout.svelte';
	import type {
		PalContainerSelection,
		PalContainerServerPaging
	} from '$states/palContainer.svelte';
	import { cn } from '$theme';
	import {
		getUpsState,
		getModalState,
		getAppState,
		getPalEditorState,
		getToastState
	} from '$states';
	import { elementsData, palsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { staticIcons } from '$types/icons';
	import {
		type Pal,
		type UPSPal,
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

	import UPSPalBadge from './components/UPSPalBadge.svelte';
	import UPSCollectionsPanel from './components/UPSCollectionsPanel.svelte';
	import UPSTagsPanel from './components/UPSTagsPanel.svelte';
	import UPSStatsPanel from './components/UPSStatsPanel.svelte';
	import { buildUpsActions } from './upsActions';

	const upsState = getUpsState();
	const modal = getModalState();
	const appState = getAppState();
	const palEditor = getPalEditorState();
	const toast = getToastState();

	const openPanel = $derived(
		upsState.showCollectionsPanel
			? { title: m.collection({ count: 2 }) }
			: upsState.showTagsPanel
				? { title: c.tags }
				: upsState.showStatsPanel
					? { title: m.statistics() }
					: null
	);

	// The collections panel opens by default, which on a phone covers the grid.
	$effect(() => {
		if (!layout.phone) return;
		untrack(() => upsState.closePanels());
	});

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

	// `pageSize` must be passed: the view's default of 30 would mismatch server pages.
	// Getters so every read reaches the current `pagination`.
	const serverPaging: PalContainerServerPaging = {
		get page() {
			return upsState.pagination.page;
		},
		get totalCount() {
			return upsState.pagination.totalCount;
		},
		get pageSize() {
			return upsState.pagination.limit;
		},
		onPageChange: (page: number) => {
			upsState.setPage(page);
			upsState.loadPals();
		}
	};

	// Owned by the state object: it spans pages the view has never been handed.
	const selection: PalContainerSelection<number> = {
		get ids() {
			return upsState.selectedPals;
		},
		onToggle: (id: number) => upsState.togglePalSelection(id)
	};

	const upsActions = $derived(
		buildUpsActions({
			selectionCount: upsState.selectedPals.size,
			pageCount: upsState.pals.length,
			totalCount: upsState.pagination.totalCount,
			filtered: hasActiveFilters(),
			selectAllOnPage: () => upsState.selectAllPals(),
			selectAllMatching: () => upsState.selectAllFilteredPals(),
			editTags: handleBulkEditTags,
			addToCollection: handleBulkAddToCollection,
			exportSelected: handleBulkExport,
			deleteSelected,
			clearSelection: () => upsState.clearSelection()
		})
	);

	function handleSort(sortBy: UPSSortBy) {
		const newOrder: UPSSortOrder =
			upsState.filters.sortBy === sortBy && upsState.filters.sortOrder === 'asc' ? 'desc' : 'asc';

		upsState.updateSort(sortBy, newOrder);
		upsState.loadPals(true);
	}

	function getSortIcon(sortBy: UPSSortBy) {
		if (upsState.filters.sortBy !== sortBy) {
			return 'tabler:arrows-sort';
		}
		return upsState.filters.sortOrder === 'asc'
			? 'tabler:sort-ascending-numbers'
			: 'tabler:sort-descending-numbers';
	}

	function clearFilters() {
		searchInput = '';
		upsState.clearFilters();
		upsState.loadPals(true);
	}

	function handleElementTypeFilter(elementType: string) {
		const currentTypes = [...upsState.filters.elementTypes];
		if (currentTypes.includes(elementType)) {
			const newTypes = currentTypes.filter((t) => t !== elementType);
			upsState.updateElementTypesFilter(newTypes);
		} else {
			currentTypes.push(elementType);
			upsState.updateElementTypesFilter(currentTypes);
		}
		upsState.loadPals(true);
	}

	function handlePalTypeFilter(palType: string) {
		const currentTypes = [...upsState.filters.palTypes];
		if (currentTypes.includes(palType)) {
			const newTypes = currentTypes.filter((t) => t !== palType);
			upsState.updatePalTypesFilter(newTypes);
		} else {
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
	function tabPill(active: boolean): string {
		return active
			? 'bg-primary-500/15 text-primary-300 border-primary-500/40 border'
			: 'text-surface-300 hover:bg-surface-800 border border-transparent';
	}

	function getElementButtonClass(element: string) {
		return cn('btn', upsState.filters.elementTypes.includes(element) ? 'bg-secondary-500/25' : '');
	}

	function getPalTypeButtonClass(palType: string) {
		return cn('btn', upsState.filters.palTypes.includes(palType) ? 'bg-secondary-500/25' : '');
	}

	function hasActiveFilters(): boolean {
		return (
			!!upsState.filters.search ||
			!!upsState.filters.collectionId ||
			upsState.filters.tags.length > 0 ||
			upsState.filters.elementTypes.length > 0 ||
			upsState.filters.palTypes.length > 0
		);
	}

	function nicknameOf(upsPal: UPSPal): string {
		return upsPal.nickname || upsPal.character_id;
	}

	function formatDate(dateString: string): string {
		return new Date(dateString).toLocaleString();
	}

	// Assigned before `open` so the `__ups_*` markers dodge excess-property checks.
	function handleOpenPal(upsPal: UPSPal): void {
		// Via `Partial<Pal>`: svelte-check rejects a bare `as Pal` on the spread.
		const pal = { ...(upsPal.pal_data as Partial<Pal>), id: upsPal.id } as Pal;
		if (!pal.character_key && upsPal.character_key) {
			pal.character_key = upsPal.character_key;
		}
		const palWithMetadata = {
			...pal,
			__ups_source: true,
			__ups_id: upsPal.id
		};
		palEditor.open(palWithMetadata);
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
			for (const palId of selectedPalIds) {
				await upsState.updatePal(palId, { tags: result });
			}

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
			const collectionId = result.removeFromCollection ? undefined : result.collectionId;

			for (const palId of selectedPalIds) {
				await upsState.updatePal(palId, { collection_id: collectionId });
			}

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

	async function handleClonePal(upsPal: UPSPal) {
		const confirmed = await modal.showConfirmModal({
			title: m.clone_selected_pal({ pal: c.pal }),
			message: m.clone_pal_to_entity({ pal: nicknameOf(upsPal), entity: c.universalPalStorage }),
			confirmText: m.clone_selected_pal({ pal: '' }),
			cancelText: m.cancel()
		});

		if (confirmed) {
			await upsState.clonePal(upsPal.id);
		}
	}

	async function handleExportPal(upsPal: UPSPal) {
		// @ts-ignore
		const result = await modal.showModal<{ target: string; playerId?: string }>(ExportPalModal, {
			title: m.export_pals({ pals: c.pal, count: 1 }),
			pals: [upsPal]
		});

		if (!result) return;

		const target = result.target as 'pal_box' | 'dps' | 'gps';
		try {
			await upsState.exportPal(upsPal.id, target, result.playerId);
			toast.add(
				m.successfully_cloned_pal_to_entity({
					pal: nicknameOf(upsPal),
					entity: result.target.toUpperCase()
				}),
				m.success(),
				'success'
			);
		} catch (error) {
			console.error('Export failed:', error);
			toast.add(m.import_failed(), m.error(), 'error');
		}
	}

	async function handleAddPalToCollection(upsPal: UPSPal) {
		// @ts-ignore
		const result = await modal.showModal<AddToCollectionResult>(AddToCollectionModal, {
			title: m.add_to_collection(),
			pals: [upsPal]
		});

		if (!result) return;

		const collectionId = result.removeFromCollection ? undefined : result.collectionId;
		await upsState.updatePal(upsPal.id, { collection_id: collectionId });
		await upsState.loadAll();
	}

	async function handleEditPalTags(upsPal: UPSPal) {
		// @ts-ignore
		const result = await modal.showModal<string[]>(EditTagsModal, {
			title: m.edit_entity({ entity: c.tags }),
			pals: [upsPal]
		});

		if (!result) return;

		await upsState.updatePal(upsPal.id, { tags: result });
		await upsState.loadAll();
	}

	async function handleDeletePal(upsPal: UPSPal) {
		const confirmed = await modal.showConfirmModal({
			title: m.delete_entity({ entity: c.pal }),
			message: m.delete_entity_by_name_confirm({ name: nicknameOf(upsPal) }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});

		if (confirmed) {
			await upsState.deletePals([upsPal.id]);
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
			let character_key = selectedPal.toLowerCase();
			if (character_key.startsWith('boss_')) {
				character_key = character_key.slice(5);
			} else if (character_key.startsWith('predator_')) {
				character_key = character_key.slice(9);
			} else if (character_key.endsWith('_avatar')) {
				character_key = character_key.slice(0, -7);
			}

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
				is_awakened: false,
				is_imported: false,
				is_tower: false,
				is_sick: false,
				gender: PalGender.MALE,
				friendship_point: 0,
				storage_id: '00000000-0000-0000-0000-000000000000',
				storage_slot: 0,
				group_id: null,
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

	// Mirrors `UPSPalBadge`'s context menu, which touch devices cannot open.
	function palActions(upsPal: UPSPal): ActionDescriptor[] {
		return [
			{
				id: 'ups-pal-clone',
				label: m.clone_selected_pal({ pal: c.pal }),
				icon: 'tabler:copy',
				run: () => handleClonePal(upsPal)
			},
			{
				id: 'ups-pal-export',
				label: m.export_pals({ pals: c.pal, count: 1 }),
				icon: 'tabler:upload',
				run: () => handleExportPal(upsPal)
			},
			{
				id: 'ups-pal-add-to-collection',
				label: m.add_to_collection(),
				icon: 'tabler:folder-plus',
				run: () => handleAddPalToCollection(upsPal)
			},
			{
				id: 'ups-pal-edit-tags',
				label: m.edit_entity({ entity: c.tags }),
				icon: 'tabler:tag',
				run: () => handleEditPalTags(upsPal)
			},
			{
				id: 'ups-pal-delete',
				label: m.delete_entity({ entity: c.pal }),
				icon: 'tabler:trash',
				run: () => handleDeletePal(upsPal),
				danger: true
			}
		];
	}

	$effect(() => {
		searchInput = upsState.filters.search;
	});
</script>

<!-- The page's own search box: UPS searches on the server, so no `matches` predicate. -->
{#snippet filters()}
	<div id="ups-filters" class="flex flex-col gap-4">
		<Input
			bind:value={searchInput}
			oninput={handleSearchInput}
			placeholder={m.search_entity({ entity: c.pals })}
		/>

		<Accordion base="w-full" collapsible>
			<Accordion.Item
				value="filters"
				base="rounded-sm bg-surface-900"
				controlHover="hover:bg-secondary-500/25"
			>
				{#snippet control()}
					<div class="flex items-center gap-2">
						<Icon icon="tabler:filter" class="h-4 w-4" />
						<span class="font-bold">{m.filter_and_sort()}</span>
						{#if hasActiveFilters()}
							<span class="bg-secondary-500/25 text-secondary-300 rounded-full px-2 py-0.5 text-xs"
								>{m.active()}</span
							>
						{/if}
					</div>
				{/snippet}
				{#snippet panel()}
					<div class="flex flex-col gap-4">
						<div class="space-y-4">
							<legend class="font-bold">{m.element_and_type()}</legend>
							<div class="space-y-3">
								<div>
									<div class="mb-1 flex items-center justify-between">
										<span class="text-surface-400 text-xs font-medium">
											{m.element_types()}
										</span>
										{#if upsState.filters.elementTypes.length > 0}
											<button
												class="text-primary-400 hover:text-primary-300 text-xs"
												onclick={clearElementTypeFilters}
											>
												{m.clear()} ({upsState.filters.elementTypes.length})
											</button>
										{/if}
									</div>
									<div class="grid grid-cols-3 gap-1 sm:grid-cols-4 md:grid-cols-5">
										{#each elementTypes as element}
											{@const elementData = elementsData.getByKey(element)}
											{@const localizedName = elementData?.localized_name || element}
											<Tooltip label={localizedName}>
												<button
													class={getElementButtonClass(element)}
													onclick={() => handleElementTypeFilter(element)}
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
									</div>
								</div>

								<div>
									<div class="mb-1 flex items-center justify-between">
										<span class="text-surface-400 text-xs font-medium">
											{m.pal_types()}
										</span>
										{#if upsState.filters.palTypes.length > 0}
											<button
												class="text-primary-400 hover:text-primary-300 text-xs"
												onclick={clearPalTypeFilters}
											>
												{m.clear()} ({upsState.filters.palTypes.length})
											</button>
										{/if}
									</div>
									<div class="grid grid-cols-3 gap-1 sm:grid-cols-4 md:grid-cols-6">
										<TooltipButton
											popupLabel={m.alpha_pal({ pals: c.pals })}
											onclick={() => handlePalTypeFilter('alpha')}
											buttonClass={getPalTypeButtonClass('alpha')}
										>
											<img src={staticIcons.alphaIcon} alt="Alpha" class="pal-element-badge" />
										</TooltipButton>
										<TooltipButton
											popupLabel={m.lucky_pals({ pals: c.pals })}
											onclick={() => handlePalTypeFilter('lucky')}
											buttonClass={getPalTypeButtonClass('lucky')}
										>
											<img src={staticIcons.luckyIcon} alt="Lucky" class="pal-element-badge" />
										</TooltipButton>
										<TooltipButton
											popupLabel={m.awakened()}
											onclick={() => handlePalTypeFilter('awakened')}
											buttonClass={getPalTypeButtonClass('awakened')}
										>
											<img
												src={staticIcons.awakeningIcon}
												alt="Awakened"
												class="pal-element-badge"
											/>
										</TooltipButton>
										<TooltipButton
											popupLabel={m.imported()}
											onclick={() => handlePalTypeFilter('imported')}
											buttonClass={getPalTypeButtonClass('imported')}
										>
											<img
												src={staticIcons.importedIcon}
												alt="Imported"
												class="pal-element-badge"
											/>
										</TooltipButton>
										<TooltipButton
											popupLabel={m.human({ count: 2 })}
											buttonClass={getPalTypeButtonClass('human')}
											onclick={() => handlePalTypeFilter('human')}
										>
											<Icon icon="tabler:user" />
										</TooltipButton>
										<TooltipButton
											popupLabel={m.predator_pals({ pals: c.pals })}
											buttonClass={getPalTypeButtonClass('predator')}
											onclick={() => handlePalTypeFilter('predator')}
										>
											<img
												src={staticIcons.predatorIcon}
												alt="Predator"
												class="pal-element-badge"
											/>
										</TooltipButton>
										<TooltipButton
											popupLabel={m.oil_rig_pals({ pals: c.pals })}
											buttonClass={getPalTypeButtonClass('oilrig')}
											onclick={() => handlePalTypeFilter('oilrig')}
										>
											<img src={staticIcons.oilrigIcon} alt="Oil Rig" class="pal-element-badge" />
										</TooltipButton>
										<TooltipButton
											popupLabel={m.summoned_pals({ pals: c.pals })}
											buttonClass={getPalTypeButtonClass('summon')}
											onclick={() => handlePalTypeFilter('summon')}
										>
											<img src={staticIcons.altarIcon} alt="Summoned" class="pal-element-badge" />
										</TooltipButton>
									</div>
								</div>
							</div>
						</div>

						<div>
							<legend class="mb-2 font-bold">{m.sort_by()}</legend>
							<div class="flex flex-wrap gap-2">
								{#each [{ key: 'created_at', label: m.created() }, { key: 'updated_at', label: m.modified() }, { key: 'character_id', label: m.character() }, { key: 'nickname', label: m.name() }, { key: 'level', label: m.level() }, { key: 'transfer_count', label: m.transfer( { count: 2 } ) }, { key: 'clone_count', label: m.clones() }] as sortOption}
									{@const sortIcon = getSortIcon(sortOption.key as UPSSortBy)}
									<button
										class={cn(
											'btn btn-sm',
											upsState.filters.sortBy === sortOption.key ? 'bg-secondary-500/25' : ''
										)}
										onclick={() => handleSort(sortOption.key as UPSSortBy)}
									>
										{sortOption.label}
										<Icon icon={sortIcon} class="h-3 w-3" />
									</button>
								{/each}
							</div>
						</div>

						{#if hasActiveFilters()}
							<div class="border-surface-700/60 border-t pt-2">
								<button
									onclick={clearFilters}
									class="text-primary-400 hover:text-primary-300 flex items-center gap-1 text-sm"
								>
									<Icon icon="tabler:x" class="h-3 w-3" />
									{m.clear_all_entity({ entity: m.filter({ count: 2 }) })}
								</button>
							</div>
						{/if}
					</div>
				{/snippet}
			</Accordion.Item>
		</Accordion>
	</div>
{/snippet}

{#snippet portrait(upsPal: UPSPal)}
	<div class="relative">
		<UPSPalBadge {upsPal} />

		{#if upsPal.tags && upsPal.tags.length > 0}
			<div
				class="bg-surface-950/80 text-surface-100 pointer-events-none absolute right-0 -bottom-1 rounded px-1 py-0.5 text-[10px] leading-none backdrop-blur-sm"
			>
				{upsPal.tags.length}<Icon icon="tabler:tag" size={10} class="ml-0.5 inline" />
			</div>
		{/if}

		{#if upsPal.transfer_count > 0 || upsPal.clone_count > 0}
			<div
				class="bg-surface-950/80 text-surface-100 pointer-events-none absolute -top-1 right-0 space-y-0.5 rounded px-1 py-0.5 text-right text-[10px] leading-none backdrop-blur-sm"
			>
				{#if upsPal.transfer_count > 0}
					<div title={m.transfer({ count: 2 })}>
						<Icon icon="tabler:upload" size={10} class="mr-0.5 inline" />{upsPal.transfer_count}
					</div>
				{/if}
				{#if upsPal.clone_count > 0}
					<div title={m.clones()}>
						<Icon icon="tabler:refresh" size={10} class="mr-0.5 inline" />{upsPal.clone_count}
					</div>
				{/if}
			</div>
		{/if}
	</div>
{/snippet}

{#snippet columns(upsPal: UPSPal)}
	<div class="flex min-w-0 flex-col gap-0.5">
		<div class="flex min-w-0 items-center gap-2">
			<span class="truncate text-sm font-bold">{nicknameOf(upsPal)}</span>
			{#if upsPal.nickname && upsPal.nickname !== upsPal.character_id}
				<span class="text-surface-400 truncate text-xs">({upsPal.character_id})</span>
			{/if}
			<span class="text-surface-300 shrink-0 text-xs">
				{m.level_abbr()}. {upsPal.level}
			</span>
		</div>

		{#if upsPal.tags && upsPal.tags.length > 0}
			<div class="flex flex-wrap gap-1">
				{#each upsPal.tags.slice(0, 3) as tag}
					<span
						class="bg-secondary-500/20 text-secondary-300 inline-flex items-center rounded px-2 py-0.5 text-xs font-medium"
					>
						{tag}
					</span>
				{/each}
				{#if upsPal.tags.length > 3}
					<span class="text-surface-400 text-xs"
						>{m.and_more_count({ count: upsPal.tags.length - 3 })}</span
					>
				{/if}
			</div>
		{/if}

		{#if upsPal.notes}
			<p class="text-surface-400 truncate text-xs">{upsPal.notes}</p>
		{/if}

		<span class="text-surface-500 truncate text-xs">
			{#if upsPal.source_save_file}
				{m.origin_label()}
				{upsPal.source_save_file} ·
			{/if}
			{m.added_label()}
			{formatDate(upsPal.created_at)}
		</span>
	</div>
{/snippet}

{#snippet detail(upsPal: UPSPal)}
	<dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
		<dt class="text-surface-400">{m.nickname()}</dt>
		<dd class="truncate">{nicknameOf(upsPal)}</dd>
		<dt class="text-surface-400">{m.character()}</dt>
		<dd class="truncate">{upsPal.character_id}</dd>
		<dt class="text-surface-400">{m.level()}</dt>
		<dd>{upsPal.level}</dd>
		<dt class="text-surface-400">{m.transfer({ count: 2 })}</dt>
		<dd>{upsPal.transfer_count}</dd>
		<dt class="text-surface-400">{m.clones()}</dt>
		<dd>{upsPal.clone_count}</dd>
		{#if upsPal.source_save_file}
			<dt class="text-surface-400">{m.origin_label()}</dt>
			<dd class="truncate">{upsPal.source_save_file}</dd>
		{/if}
		<dt class="text-surface-400">{m.added_label()}</dt>
		<dd>{formatDate(upsPal.created_at)}</dd>
		{#if upsPal.updated_at !== upsPal.created_at}
			<dt class="text-surface-400">{m.modified_label()}</dt>
			<dd>{formatDate(upsPal.updated_at)}</dd>
		{/if}
		{#if upsPal.last_accessed_at}
			<dt class="text-surface-400">{m.last_accessed()}</dt>
			<dd>{formatDate(upsPal.last_accessed_at)}</dd>
		{/if}
		{#if upsPal.tags && upsPal.tags.length > 0}
			<dt class="text-surface-400">{c.tags}</dt>
			<dd>{upsPal.tags.join(', ')}</dd>
		{/if}
		{#if upsPal.notes}
			<dt class="text-surface-400">{m.note()}</dt>
			<dd>{upsPal.notes}</dd>
		{/if}
	</dl>
{/snippet}

<div class="animate-fade-in flex h-full flex-col">
	<div id="ups-header" class="flex flex-wrap items-center justify-between gap-3 px-4 pt-4">
		<div class="flex min-w-0 items-center gap-2">
			<Icon icon="tabler:database" size={20} class="text-primary-400" />
			<div class="min-w-0">
				<h1 class="heading-gradient text-xl font-bold">
					{m.universal_pal_storage({ pal: c.pal })}
				</h1>
				<p class="text-surface-400 text-sm">
					{upsState.pagination.totalCount}
					{m.pals_in_storage({ pals: m.pal({ count: upsState.pagination.totalCount }) })}
				</p>
			</div>
		</div>

		<div class="flex flex-wrap items-center gap-2">
			<TooltipButton
				onclick={handleAddPal}
				variant="secondary"
				size="icon"
				popupLabel={m.add_new_pal({ pal: c.pal })}
			>
				<Icon icon="tabler:plus" class="h-4 w-4" />
			</TooltipButton>

			{#if appState.saveFile}
				<TooltipButton
					onclick={handleImportFromSave}
					variant="primary"
					size="icon"
					popupLabel={m.import_from_save()}
				>
					<Icon icon="tabler:upload" class="h-4 w-4" />
				</TooltipButton>
			{/if}

			{#if upsState.pagination.totalCount > 0}
				<TooltipButton popupLabel={m.nuke_ups({ pals: c.pals })}>
					<button
						class="tap-target text-error-500 hover:bg-error-500/20 hover:text-error-400 h-8 w-8 rounded-md p-2 transition-colors"
						onclick={handleNukeUps}
						disabled={upsState.loading}
					>
						<Nuke size={16} />
					</button>
				</TooltipButton>
			{/if}

			{#if upsState.pagination.totalCount > 0 || appState.saveFile}
				<div class="bg-surface-700 h-6 w-px"></div>
			{/if}

			<div class="bg-surface-950/50 border-surface-700/40 flex gap-1 rounded-sm border p-0.5">
				<TooltipButton
					onclick={() => upsState.toggleCollectionsPanel()}
					class="rounded-sm p-2 transition-all {tabPill(upsState.showCollectionsPanel)}"
					popupLabel={m.toggle_entity({ entity: m.collection({ count: 2 }) })}
				>
					<Icon icon="tabler:folder" class="h-4 w-4" />
				</TooltipButton>

				<TooltipButton
					onclick={() => upsState.toggleTagsPanel()}
					class="rounded-sm p-2 transition-all {tabPill(upsState.showTagsPanel)}"
					popupLabel={m.toggle_entity({ entity: c.tags })}
				>
					<Icon icon="tabler:tag" class="h-4 w-4" />
				</TooltipButton>

				<TooltipButton
					onclick={() => upsState.toggleStatsPanel()}
					class="rounded-sm p-2 transition-all {tabPill(upsState.showStatsPanel)}"
					popupLabel={m.toggle_entity({ entity: m.statistics() })}
				>
					<Icon icon="tabler:chart-bar" class="h-4 w-4" />
				</TooltipButton>
			</div>
		</div>
	</div>

	<div class="flex flex-1 overflow-hidden">
		{#if openPanel}
			<DetailPresentation
				class="animate-slide-down flex w-full flex-col gap-2 p-4 sm:w-72 sm:flex-none md:w-80"
				title={openPanel.title}
				onClose={() => upsState.closePanels()}
			>
				<div id="ups-panels" class="flex flex-col gap-2">
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
			</DetailPresentation>
		{/if}

		<div class="flex min-h-0 min-w-0 flex-1 flex-col p-2">
			{#if upsState.loading}
				<div class="flex h-full flex-col items-center justify-center gap-4">
					<Spinner size="size-16" />
					<p class="text-surface-400">
						{m.loading_entity({ entity: m.universal_pal_storage({ pal: c.pal }) })}
					</p>
				</div>
				<!-- A filter-emptied page keeps the container so its filter button stays reachable. -->
			{:else if upsState.pals.length === 0 && !hasActiveFilters()}
				<div class="flex h-64 flex-col items-center justify-center text-center">
					<Icon icon="tabler:user" class="text-surface-500 mb-4 h-16 w-16" />
					<h3 class="text-surface-300 mb-2 text-lg font-medium">
						{m.no_pals_in_storage({ pals: c.pals })}
					</h3>
					<p class="text-surface-400 mb-4 max-w-md">
						{m.create_pals_or_import({ pals: c.pals })}
					</p>
					<div class="flex flex-wrap justify-center gap-3">
						<Button variant="secondary" onclick={handleAddPal}>
							<Icon icon="tabler:plus" class="h-4 w-4" />
							{m.add_new_pal({ pal: c.pal })}
						</Button>
						{#if appState.saveFile}
							<Button variant="secondary" onclick={handleImportFromSave}>
								<Icon icon="tabler:upload" class="h-4 w-4" />
								{m.import_from_save()}
							</Button>
						{/if}
					</div>
				</div>
			{:else}
				<div class="flex min-h-0 gap-2">
					<!-- With a selection, the view's own toolbar carries these rows. -->
					{#if upsState.selectedPals.size === 0}
						<ActionGroup id="ups-actions" actions={upsActions} title={m.quick_actions()} />
					{/if}

					<div class="min-w-0 flex-1">
						<PalContainerView
							pals={upsState.pals}
							idOf={(upsPal) => upsPal.id}
							{nicknameOf}
							storageKey="ups"
							title={c.pals}
							actions={upsActions}
							{serverPaging}
							{selection}
							{filters}
							{portrait}
							{columns}
							{detail}
							{palActions}
							onOpenPal={handleOpenPal}
						/>
					</div>
				</div>
			{/if}
		</div>
	</div>
</div>

<style>
	.pal-element-badge {
		width: 24px;
		height: 24px;
	}
</style>
