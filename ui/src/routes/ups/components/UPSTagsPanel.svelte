<script lang="ts">
	import { Plus, Tag, X, Edit, Trash2 } from 'lucide-svelte';
	import { getUpsState, getModalState } from '$states';
	import { TooltipButton } from '$components/ui';
	import { TextInputModal } from '$components';
	import type { UPSTag } from '$types';

	const upsState = getUpsState();
	const modal = getModalState();

	let searchTags = $state('');

	const filteredTags = $derived(
		upsState.availableTags.filter((tag) =>
			tag.name.toLowerCase().includes(searchTags.toLowerCase())
		)
	);

	async function createTag() {
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: 'Create Tag',
			value: '',
			inputLabel: 'Enter a name for the new tag:'
		});

		if (result) {
			await upsState.createTag(result);
		}
	}

	async function editTag(tag: UPSTag) {
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: 'Edit Tag',
			value: tag.name,
			inputLabel: 'Enter new name for the tag:'
		});

		if (result && result !== tag.name) {
			await upsState.updateTag(tag.id, { name: result });
		}
	}

	async function deleteTag(tag: UPSTag) {
		const confirmed = await modal.showConfirmModal({
			title: 'Delete Tag',
			message: `Are you sure you want to delete "${tag.name}"? This tag will be removed from all Pals that have it.`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});

		if (confirmed) {
			await upsState.deleteTag(tag.id);
		}
	}

	function toggleTagFilter(tagName: string) {
		const currentTags = [...upsState.filters.tags];
		const index = currentTags.indexOf(tagName);

		if (index >= 0) {
			currentTags.splice(index, 1);
		} else {
			currentTags.push(tagName);
		}

		upsState.updateTagFilter(currentTags);
		upsState.loadPals(true);
	}

	function isTagSelected(tagName: string): boolean {
		return upsState.filters.tags.includes(tagName);
	}

	function clearTagFilters() {
		upsState.updateTagFilter([]);
		upsState.loadPals(true);
	}
</script>

<div class="flex h-full flex-col">
	<!-- Header -->
	<div class="border-surface-300 dark:border-surface-700 border-b p-4">
		<div class="mb-3 flex items-center justify-between">
			<h2 class="text-lg font-semibold">Tags</h2>
			<TooltipButton
				onclick={createTag}
				class="bg-primary-500 hover:bg-primary-600 rounded-md p-2 text-white"
				popupLabel="Create Tag"
			>
				<Plus class="h-4 w-4" />
			</TooltipButton>
		</div>

		<!-- Search -->
		<div class="relative">
			<input
				type="text"
				bind:value={searchTags}
				placeholder="Search tags..."
				class="border-surface-300 dark:border-surface-600 dark:bg-surface-800 w-full rounded-md border bg-white py-2 pl-8 pr-3 text-sm"
			/>
			<Tag class="text-surface-500 absolute left-2.5 top-1/2 h-3 w-3 -translate-y-1/2" />
		</div>

		<!-- Active Filter Summary -->
		{#if upsState.filters.tags.length > 0}
			<div class="mt-3">
				<div class="mb-2 flex items-center justify-between">
					<span
						class="text-surface-600 dark:text-surface-400 text-xs font-medium uppercase tracking-wide"
					>
						Active Filters ({upsState.filters.tags.length})
					</span>
					<button onclick={clearTagFilters} class="text-primary-600 hover:text-primary-700 text-xs">
						Clear All
					</button>
				</div>
				<div class="flex flex-wrap gap-1">
					{#each upsState.filters.tags as tagName}
						<span
							class="bg-primary-100 text-primary-800 dark:bg-primary-900/30 dark:text-primary-300 inline-flex items-center gap-1 rounded px-2 py-1 text-xs"
						>
							{tagName}
							<button onclick={() => toggleTagFilter(tagName)} class="hover:text-primary-600">
								<X class="h-3 w-3" />
							</button>
						</span>
					{/each}
				</div>
			</div>
		{/if}
	</div>

	<!-- Tags List -->
	<div class="flex-1 overflow-auto p-4">
		{#if filteredTags.length > 0}
			<div class="space-y-1">
				{#each filteredTags as tag (tag.id)}
					{@const isSelected = isTagSelected(tag.name)}
					<div class="group relative">
						<button
							onclick={() => toggleTagFilter(tag.name)}
							class="hover:bg-surface-200 dark:hover:bg-surface-700 flex w-full items-center gap-3 rounded-md p-2 text-left transition-colors {isSelected
								? 'bg-primary-500 text-white'
								: ''}"
						>
							<!-- Tag Color Indicator -->
							<div
								class="h-3 w-3 flex-shrink-0 rounded-full"
								style="background-color: {tag.color || '#6366f1'}"
							></div>

							<!-- Tag Name -->
							<span class="flex-1 truncate font-medium">
								{tag.name}
							</span>

							<!-- Usage Count -->
							<span
								class="flex-shrink-0 text-xs {isSelected ? 'text-primary-100' : 'text-surface-500'}"
							>
								{tag.usage_count}
							</span>
						</button>

						<!-- Action buttons (show on hover) -->
						<div
							class="absolute right-1 top-1 flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100"
						>
							<TooltipButton
								onclick={(e: MouseEvent) => {
									e.stopPropagation();
									editTag(tag);
								}}
								class="rounded bg-black/20 p-1 text-white hover:bg-black/40"
								popupLabel="Edit tag"
								size="sm"
							>
								<Edit class="h-3 w-3" />
							</TooltipButton>
							<TooltipButton
								onclick={(e: MouseEvent) => {
									e.stopPropagation();
									deleteTag(tag);
								}}
								class="rounded bg-red-500/80 p-1 text-white hover:bg-red-600/80"
								popupLabel="Delete tag"
								size="sm"
							>
								<Trash2 class="h-3 w-3" />
							</TooltipButton>
						</div>
					</div>
				{/each}
			</div>
		{:else if searchTags}
			<!-- No Search Results -->
			<div class="py-8 text-center">
				<Tag class="text-surface-400 mx-auto mb-3 h-12 w-12" />
				<p class="text-surface-500 mb-2 text-sm">
					No tags found matching "{searchTags}"
				</p>
				<button onclick={createTag} class="text-primary-600 hover:text-primary-700 text-sm">
					Create "{searchTags}" tag
				</button>
			</div>
		{:else if upsState.availableTags.length === 0}
			<!-- Empty State -->
			<div class="py-8 text-center">
				<Tag class="text-surface-400 mx-auto mb-3 h-12 w-12" />
				<p class="text-surface-500 mb-2 text-sm">No tags yet</p>
				<button onclick={createTag} class="text-primary-600 hover:text-primary-700 text-sm">
					Create your first tag
				</button>
			</div>
		{/if}
	</div>

	<!-- Footer Info -->
	{#if upsState.availableTags.length > 0}
		<div class="border-surface-300 dark:border-surface-700 border-t p-4">
			<p class="text-surface-500 text-center text-xs">
				{upsState.availableTags.length} tag{upsState.availableTags.length > 1 ? 's' : ''} available
			</p>
		</div>
	{/if}
</div>
