<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { getUpsState, getModalState } from '$states';
	import { Input, TooltipButton } from '$components/ui';
	import TextInputModal from '$components/modals/text-input/TextInputModal.svelte';
	import type { UPSTag } from '$types';
	import * as m from '$i18n/messages';
	import { c } from '$utils/commonTranslations';

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
			title: m.add_entity({ entity: c.tag }),
			value: '',
			inputLabel: m.enter_name_for_entity({ entity: c.tag })
		});

		if (result) {
			await upsState.createTag(result);
		}
	}

	async function editTag(tag: UPSTag) {
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: m.edit_entity({ entity: c.tag }),
			value: tag.name,
			inputLabel: m.enter_new_name_for_entity({ entity: c.tag })
		});

		if (result && result !== tag.name) {
			await upsState.updateTag(tag.id, { name: result });
		}
	}

	async function deleteTag(tag: UPSTag) {
		const confirmed = await modal.showConfirmModal({
			title: m.delete_entity({ entity: c.tag }),
			message: m.delete_entity_warning({ name: tag.name, warning: m.tag_delete_warning() }),
			confirmText: m.delete(),
			cancelText: m.cancel()
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

<div class="bg-surface-900/60 flex h-full min-h-0 flex-col overflow-hidden rounded-sm">
	<div class="border-surface-700/40 mb-2 border-b px-4 pt-4 pb-2">
		<div class="flex items-center justify-between">
			<h2 class="text-surface-100 text-sm font-bold tracking-wide uppercase">{c.tags}</h2>
			<TooltipButton
				onclick={createTag}
				variant="ghost"
				size="icon"
				class="hover:bg-secondary-500/25 text-secondary-300"
				popupLabel={m.add_entity({ entity: c.tag })}
			>
				<Icon icon="tabler:plus" class="h-4 w-4" />
			</TooltipButton>
		</div>

		<div class="relative">
			<Input
				type="text"
				bind:value={searchTags}
				inputClass="w-full pl-7 my-1"
				placeholder={m.search_placeholder({ entity: c.tags })}
			/>
			<Icon
				icon="tabler:tag"
				class="text-surface-500 pointer-events-none absolute bottom-3 left-3 h-3 w-3"
			/>
		</div>

		{#if upsState.filters.tags.length > 0}
			<div class="mt-3">
				<div class="mb-2 flex items-center justify-between">
					<span class="text-muted text-xs font-bold tracking-wider uppercase">
						{m.active_filters_count({ count: upsState.filters.tags.length })}
					</span>
					<button onclick={clearTagFilters} class="text-primary-400 hover:text-primary-300 text-xs">
						{m.clear_all()}
					</button>
				</div>
				<div class="flex flex-wrap gap-1">
					{#each upsState.filters.tags as tagName}
						<span
							class="bg-secondary-500/20 text-secondary-300 inline-flex items-center gap-1 rounded px-2 py-1 text-xs"
						>
							{tagName}
							<button onclick={() => toggleTagFilter(tagName)} class="hover:text-error-400">
								<Icon icon="tabler:x" class="h-3 w-3" />
							</button>
						</span>
					{/each}
				</div>
			</div>
		{/if}
	</div>

	<div class="flex-1 overflow-auto px-4 pb-4">
		{#if filteredTags.length > 0}
			<div class="space-y-1">
				{#each filteredTags as tag (tag.id)}
					{@const isSelected = isTagSelected(tag.name)}
					<div class="group relative">
						<button
							onclick={() => toggleTagFilter(tag.name)}
							class="hover:bg-secondary-500/25 flex w-full items-center gap-3 rounded-sm p-2 text-left transition-colors {isSelected
								? 'bg-secondary-500/25'
								: ''}"
						>
							<div
								class="h-3 w-3 shrink-0 rounded-full"
								style="background-color: {tag.color || '#6366f1'}"
							></div>

							<span class="flex-1 truncate font-medium">
								{tag.name}
							</span>

							<span class="text-muted shrink-0 text-xs">
								{tag.usage_count}
							</span>
						</button>

						<div
							class="absolute top-1 right-1 flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100"
						>
							<TooltipButton
								onclick={(e: MouseEvent) => {
									e.stopPropagation();
									editTag(tag);
								}}
								class="bg-surface-950/80 text-surface-200 hover:bg-surface-700 rounded p-1 backdrop-blur-sm"
								popupLabel={m.edit_entity({ entity: c.tag })}
								size="sm"
							>
								<Icon icon="tabler:edit" class="h-3 w-3" />
							</TooltipButton>
							<TooltipButton
								onclick={(e: MouseEvent) => {
									e.stopPropagation();
									deleteTag(tag);
								}}
								class="bg-error-500/20 text-error-300 hover:bg-error-500/40 rounded p-1"
								popupLabel={m.delete_entity({ entity: c.tag })}
								size="sm"
							>
								<Icon icon="tabler:trash-x" class="h-3 w-3" />
							</TooltipButton>
						</div>
					</div>
				{/each}
			</div>
		{:else if searchTags}
			<div class="py-8 text-center">
				<Icon icon="tabler:tag" class="text-surface-500 mx-auto mb-3 h-12 w-12" />
				<p class="text-surface-400 mb-2 text-sm">
					{m.no_entity_matching({ entity: c.tags, query: searchTags })}
				</p>
				<button onclick={createTag} class="text-primary-400 hover:text-primary-300 text-sm">
					{m.create_entity_name({ name: searchTags, entity: c.tag })}
				</button>
			</div>
		{:else if upsState.availableTags.length === 0}
			<div class="py-8 text-center">
				<Icon icon="tabler:tag" class="text-surface-500 mx-auto mb-3 h-12 w-12" />
				<p class="text-surface-400 mb-2 text-sm">{m.no_entity_yet({ entity: c.tags })}</p>
				<button onclick={createTag} class="text-primary-400 hover:text-primary-300 text-sm">
					{m.create_first_entity({ entity: c.tag })}
				</button>
			</div>
		{/if}
	</div>

	{#if upsState.availableTags.length > 0}
		<div class="border-surface-700/40 border-t px-4 py-3">
			<p class="text-surface-400 text-center text-xs">
				{m.entity_count_available({
					count: upsState.availableTags.length,
					entity: m.tag({ count: upsState.availableTags.length })
				})}
			</p>
		</div>
	{/if}
</div>
