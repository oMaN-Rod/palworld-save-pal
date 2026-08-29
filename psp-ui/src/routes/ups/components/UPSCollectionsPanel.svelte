<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { getUpsState, getModalState } from '$states';
	import { TooltipButton } from '$components/ui';
	import TextInputModal from '$components/modals/text-input/TextInputModal.svelte';
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
			message: m.delete_entity_warning({
				name: collection.name,
				warning: m.collection_delete_warning()
			}),
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

<div class="bg-surface-900/60 flex h-full min-h-0 flex-col overflow-hidden rounded-sm">
	<div class="border-surface-700/40 mb-2 border-b px-4 pt-4 pb-2">
		<div class="flex items-center justify-between">
			<h2 class="text-surface-100 text-sm font-bold tracking-wide uppercase">{c.collections}</h2>
			<TooltipButton
				onclick={createCollection}
				variant="ghost"
				size="icon"
				class="hover:bg-secondary-500/25 text-secondary-300"
				popupLabel={m.create_new_collection()}
			>
				<Icon icon="tabler:plus" class="h-4 w-4" />
			</TooltipButton>
		</div>

		<button
			onclick={() => selectCollection(null)}
			class="hover:bg-secondary-500/25 flex w-full items-center gap-2 rounded-sm p-2 text-left transition-colors {upsState
				.filters.collectionId === undefined
				? 'bg-secondary-500/25'
				: ''}"
		>
			<Icon icon="tabler:folder" class="h-4 w-4" />
			<span class="flex-1">{m.all_entity({ entity: c.pals })}</span>
			<span class="text-muted text-xs">
				{upsState.stats?.total_pals || 0}
			</span>
		</button>
	</div>

	<div class="flex-1 space-y-3 overflow-auto px-4 pb-4">
		{#if favoriteCollections.length > 0}
			<div>
				<h3 class="text-muted mb-2 text-xs font-bold tracking-wider uppercase">
					{m.favorites()}
				</h3>
				<div class="space-y-1">
					{#each favoriteCollections as collection (collection.id)}
						<div class="group relative">
							<button
								onclick={() => selectCollection(collection)}
								class="hover:bg-secondary-500/25 flex w-full items-center gap-2 rounded-sm p-2 text-left transition-colors {isCollectionSelected(
									collection.id
								)
									? 'bg-secondary-500/25'
									: ''}"
							>
								<div
									class="h-4 w-4 shrink-0 rounded"
									style="background-color: {collection.color || '#6366f1'}"
								></div>
								<span class="flex-1 truncate">{collection.name}</span>
								<span class="text-muted text-xs">
									{collection.pal_count}
								</span>
							</button>

							<div
								class="absolute top-1 right-1 flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100"
							>
								<TooltipButton
									onclick={(e: MouseEvent) => {
										e.stopPropagation();
										toggleFavorite(collection);
									}}
									class="bg-surface-950/80 text-surface-200 hover:bg-surface-700 rounded p-1 backdrop-blur-sm"
									popupLabel={m.remove_from_favorites()}
									size="sm"
								>
									<Icon icon="tabler:star" class="h-3 w-3 fill-current" />
								</TooltipButton>
								<TooltipButton
									onclick={(e: MouseEvent) => {
										e.stopPropagation();
										editCollection(collection);
									}}
									class="bg-surface-950/80 text-surface-200 hover:bg-surface-700 rounded p-1 backdrop-blur-sm"
									popupLabel={m.edit_entity({ entity: c.collection })}
									size="sm"
								>
									<Icon icon="tabler:edit" class="h-3 w-3" />
								</TooltipButton>
							</div>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		{#if regularCollections.length > 0}
			<div>
				{#if favoriteCollections.length > 0}
					<h3 class="text-muted mb-2 text-xs font-bold tracking-wider uppercase">
						{c.collections}
					</h3>
				{/if}
				<div class="space-y-1">
					{#each regularCollections as collection (collection.id)}
						<div class="group relative">
							<button
								onclick={() => selectCollection(collection)}
								class="hover:bg-secondary-500/25 flex w-full items-center gap-2 rounded-sm p-2 text-left transition-colors {isCollectionSelected(
									collection.id
								)
									? 'bg-secondary-500/25'
									: ''}"
							>
								<div
									class="h-4 w-4 shrink-0 rounded"
									style="background-color: {collection.color || '#6366f1'}"
								></div>
								<span class="flex-1 truncate">{collection.name}</span>
								<span class="text-muted text-xs">
									{collection.pal_count}
								</span>
							</button>

							<div
								class="absolute top-1 right-1 flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100"
							>
								<TooltipButton
									onclick={(e: MouseEvent) => {
										e.stopPropagation();
										toggleFavorite(collection);
									}}
									class="bg-surface-950/80 text-surface-200 hover:bg-surface-700 rounded p-1 backdrop-blur-sm"
									popupLabel={m.add_to_favorites()}
									size="sm"
								>
									<Icon icon="tabler:star" class="h-3 w-3" />
								</TooltipButton>
								<TooltipButton
									onclick={(e: MouseEvent) => {
										e.stopPropagation();
										editCollection(collection);
									}}
									class="bg-surface-950/80 text-surface-200 hover:bg-surface-700 rounded p-1 backdrop-blur-sm"
									popupLabel={m.edit_entity({ entity: c.collection })}
									size="sm"
								>
									<Icon icon="tabler:edit" class="h-3 w-3" />
								</TooltipButton>
								<TooltipButton
									onclick={(e: MouseEvent) => {
										e.stopPropagation();
										toggleArchived(collection);
									}}
									class="bg-surface-950/80 text-surface-200 hover:bg-surface-700 rounded p-1 backdrop-blur-sm"
									popupLabel={m.archive_entity({ entity: c.collection })}
									size="sm"
								>
									<Icon icon="tabler:archive" class="h-3 w-3" />
								</TooltipButton>
								<TooltipButton
									onclick={(e: MouseEvent) => {
										e.stopPropagation();
										deleteCollection(collection);
									}}
									class="bg-error-500/20 text-error-300 hover:bg-error-500/40 rounded p-1"
									popupLabel={m.delete_entity({ entity: c.collection })}
									size="sm"
								>
									<Icon icon="tabler:trash-x" class="h-3 w-3" />
								</TooltipButton>
							</div>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		{#if visibleCollections.length === 0}
			<div class="py-8 text-center">
				<Icon icon="tabler:folder" class="text-surface-500 mx-auto mb-3 h-12 w-12" />
				<p class="text-surface-400 text-sm">
					{showArchived
						? m.no_archived_entity({ entity: c.collections })
						: m.no_entity_yet({ entity: c.collections })}
				</p>
				{#if !showArchived}
					<button
						onclick={createCollection}
						class="text-primary-400 hover:text-primary-300 mt-2 text-sm"
					>
						{m.create_first_entity({ entity: c.collection })}
					</button>
				{/if}
			</div>
		{/if}
	</div>

	<div class="border-surface-700/40 border-t px-4 py-3">
		<label class="text-surface-300 flex items-center gap-2 text-sm">
			<input type="checkbox" bind:checked={showArchived} class="accent-primary-500" />
			{m.show_archived()}
		</label>
	</div>
</div>
