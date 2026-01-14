<script lang="ts">
	import { Plus, Folder, Star, Archive, Edit, Trash2 } from 'lucide-svelte';
	import { getUpsState, getModalState } from '$states';
	import { TooltipButton } from '$components/ui';
	import { TextInputModal } from '$components';
	import type { UPSCollection } from '$types';
	import * as m from '$i18n/messages';
	import { c } from '$utils/commonTranslations';

	const upsState = getUpsState();
	const modal = getModalState();

	let showArchived = $state(false);

	const visibleCollections = $derived(
		upsState.collections.filter((c) => showArchived || !c.is_archived)
	);

	const favoriteCollections = $derived(visibleCollections.filter((c) => c.is_favorite));

	const regularCollections = $derived(visibleCollections.filter((c) => !c.is_favorite));

	async function createCollection() {
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: m.create_new_collection(),
			value: '',
			inputLabel: m.enter_name_for_entity({ entity: c.collection })
		});

		if (result) {
			await upsState.createCollection(result);
		}
	}

	async function editCollection(collection: UPSCollection) {
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: m.edit_entity({ entity: c.collection }),
			value: collection.name,
			inputLabel: m.enter_new_name_for_entity({ entity: c.collection })
		});

		if (result && result !== collection.name) {
			await upsState.updateCollection(collection.id, { name: result });
		}
	}

	async function toggleFavorite(collection: UPSCollection) {
		await upsState.updateCollection(collection.id, { is_favorite: !collection.is_favorite });
	}

	async function toggleArchived(collection: UPSCollection) {
		await upsState.updateCollection(collection.id, { is_archived: !collection.is_archived });
	}

	async function deleteCollection(collection: UPSCollection) {
		const confirmed = await modal.showConfirmModal({
			title: m.delete_entity({ entity: c.collection }),
			message: m.delete_entity_warning({ name: collection.name, warning: m.collection_delete_warning() }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});

		if (confirmed) {
			await upsState.deleteCollection(collection.id);
		}
	}

	function selectCollection(collection: UPSCollection | null) {
		upsState.updateCollectionFilter(collection?.id);
		upsState.loadPals(true);
	}

	function isCollectionSelected(collectionId: number): boolean {
		return upsState.filters.collectionId === collectionId;
	}
</script>

<div class="flex h-full flex-col transition-all duration-300 ease-in-out">
	<!-- Header -->
	<div class="border-surface-300 dark:border-surface-700 border-b p-4">
		<div class="mb-3 flex items-center justify-between">
			<h2 class="text-lg font-semibold">{c.collections}</h2>
			<TooltipButton
				onclick={createCollection}
				class="bg-primary-500 hover:bg-primary-600 rounded-md p-2 text-white"
				popupLabel={m.create_new_collection()}
			>
				<Plus class="h-4 w-4" />
			</TooltipButton>
		</div>

		<!-- All Pals Option -->
		<button
			onclick={() => selectCollection(null)}
			class="hover:bg-surface-200 dark:hover:bg-surface-700 flex w-full items-center gap-2 rounded-md p-2 text-left {upsState
				.filters.collectionId === undefined
				? 'bg-primary-500 text-white'
				: ''}"
		>
			<Folder class="h-4 w-4" />
			<span class="flex-1">{m.all_entity({ entity: c.pals })}</span>
			<span class="text-surface-200 text-xs">
				{upsState.stats?.total_pals || 0}
			</span>
		</button>
	</div>

	<!-- Collections List -->
	<div class="flex-1 space-y-1 overflow-auto p-4">
		<!-- Favorites -->
		{#if favoriteCollections.length > 0}
			<div class="mb-4">
				<h3
					class="text-surface-600 dark:text-surface-400 mb-2 text-sm font-medium uppercase tracking-wide"
				>
					{m.favorites()}
				</h3>
				<div class="space-y-1">
					{#each favoriteCollections as collection (collection.id)}
						<div class="group relative">
							<button
								onclick={() => selectCollection(collection)}
								class="hover:bg-surface-200 dark:hover:bg-surface-700 flex w-full items-center gap-2 rounded-md p-2 text-left {isCollectionSelected(
									collection.id
								)
									? 'bg-primary-500 text-white'
									: ''}"
							>
								<div
									class="h-4 w-4 flex-shrink-0 rounded"
									style="background-color: {collection.color || '#6366f1'}"
								></div>
								<span class="flex-1 truncate">{collection.name}</span>
								<span class="text-surface-200 text-xs">
									{collection.pal_count}
								</span>
							</button>

							<!-- Action buttons (show on hover) -->
							<div
								class="absolute right-1 top-1 flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100"
							>
								<TooltipButton
									onclick={(e: MouseEvent) => {
										e.stopPropagation();
										toggleFavorite(collection);
									}}
									class="rounded bg-black/20 p-1 text-white hover:bg-black/40"
									popupLabel={m.remove_from_favorites()}
									size="sm"
								>
									<Star class="h-3 w-3 fill-current" />
								</TooltipButton>
								<TooltipButton
									onclick={(e: MouseEvent) => {
										e.stopPropagation();
										editCollection(collection);
									}}
									class="rounded bg-black/20 p-1 text-white hover:bg-black/40"
									popupLabel={m.edit_entity({ entity: c.collection })}
									size="sm"
								>
									<Edit class="h-3 w-3" />
								</TooltipButton>
							</div>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		<!-- Regular Collections -->
		{#if regularCollections.length > 0}
			<div class="mb-4">
				{#if favoriteCollections.length > 0}
					<h3
						class="text-surface-600 dark:text-surface-400 mb-2 text-sm font-medium uppercase tracking-wide"
					>
						{c.collections}
					</h3>
				{/if}
				<div class="space-y-1">
					{#each regularCollections as collection (collection.id)}
						<div class="group relative">
							<button
								onclick={() => selectCollection(collection)}
								class="hover:bg-surface-200 dark:hover:bg-surface-700 flex w-full items-center gap-2 rounded-md p-2 text-left {isCollectionSelected(
									collection.id
								)
									? 'bg-primary-500 text-white'
									: ''}"
							>
								<div
									class="h-4 w-4 flex-shrink-0 rounded"
									style="background-color: {collection.color || '#6366f1'}"
								></div>
								<span class="flex-1 truncate">{collection.name}</span>
								<span class="text-surface-200 text-xs">
									{collection.pal_count}
								</span>
							</button>

							<!-- Action buttons (show on hover) -->
							<div
								class="absolute right-1 top-1 flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100"
							>
								<TooltipButton
									onclick={(e: MouseEvent) => {
										e.stopPropagation();
										toggleFavorite(collection);
									}}
									class="rounded bg-black/20 p-1 text-white hover:bg-black/40"
									popupLabel={m.add_to_favorites()}
									size="sm"
								>
									<Star class="h-3 w-3" />
								</TooltipButton>
								<TooltipButton
									onclick={(e: MouseEvent) => {
										e.stopPropagation();
										editCollection(collection);
									}}
									class="rounded bg-black/20 p-1 text-white hover:bg-black/40"
									popupLabel={m.edit_entity({ entity: c.collection })}
									size="sm"
								>
									<Edit class="h-3 w-3" />
								</TooltipButton>
								<TooltipButton
									onclick={(e: MouseEvent) => {
										e.stopPropagation();
										toggleArchived(collection);
									}}
									class="rounded bg-black/20 p-1 text-white hover:bg-black/40"
									popupLabel={m.archive_entity({ entity: c.collection })}
									size="sm"
								>
									<Archive class="h-3 w-3" />
								</TooltipButton>
								<TooltipButton
									onclick={(e: MouseEvent) => {
										e.stopPropagation();
										deleteCollection(collection);
									}}
									class="rounded bg-red-500/80 p-1 text-white hover:bg-red-600/80"
									popupLabel={m.delete_entity({ entity: c.collection })}
									size="sm"
								>
									<Trash2 class="h-3 w-3" />
								</TooltipButton>
							</div>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		<!-- Empty State -->
		{#if visibleCollections.length === 0}
			<div class="py-8 text-center">
				<Folder class="text-surface-400 mx-auto mb-3 h-12 w-12" />
				<p class="text-surface-500 text-sm">
					{showArchived ? m.no_archived_entity({ entity: c.collections }) : m.no_entity_yet({ entity: c.collections })}
				</p>
				{#if !showArchived}
					<button
						onclick={createCollection}
						class="text-primary-600 hover:text-primary-700 mt-2 text-sm"
					>
						{m.create_first_entity({ entity: c.collection })}
					</button>
				{/if}
			</div>
		{/if}
	</div>

	<!-- Footer -->
	<div class="border-surface-300 dark:border-surface-700 border-t p-4">
		<label class="flex items-center gap-2 text-sm">
			<input type="checkbox" bind:checked={showArchived} class="border-surface-300 rounded" />
			{m.show_archived()}
		</label>
	</div>
</div>
