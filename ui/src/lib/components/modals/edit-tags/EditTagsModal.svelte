<script lang="ts">
	import { Card, Input, TooltipButton } from '$components/ui';
	import { Save, X, Tag, Plus } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import { getUpsState } from '$states';
	import type { UPSPal } from '$types';

	let {
		title = 'Edit Tags',
		message = '',
		pals = [],
		closeModal
	}: {
		title?: string;
		message?: string;
		pals: UPSPal[];
		closeModal: (value: any) => void;
	} = $props();

	const upsState = getUpsState();

	let modalContainer: HTMLDivElement;
	let selectedTags: string[] = $state([]);
	let newTagInput: string = $state('');

	// Get all existing tags from selected pals
	const existingTags = $derived.by(() => {
		const allTags = new Set<string>();
		pals.forEach((pal) => {
			if (pal.tags) {
				pal.tags.forEach((tag) => allTags.add(tag));
			}
		});
		return Array.from(allTags);
	});

	// Initialize selected tags with common tags across all selected pals
	$effect(() => {
		if (pals.length > 0) {
			// Find tags that are common to all selected pals
			const commonTags = existingTags.filter((tag) =>
				pals.every((pal) => pal.tags && pal.tags.includes(tag))
			);
			selectedTags = [...commonTags];
		}
	});

	function handleClose(confirmed: boolean) {
		if (!confirmed) {
			closeModal(null);
			return;
		}

		closeModal(selectedTags);
	}

	function toggleTag(tagName: string) {
		if (selectedTags.includes(tagName)) {
			selectedTags = selectedTags.filter((t) => t !== tagName);
		} else {
			selectedTags = [...selectedTags, tagName];
		}
	}

	function addNewTag() {
		if (newTagInput.trim() && !selectedTags.includes(newTagInput.trim())) {
			selectedTags = [...selectedTags, newTagInput.trim()];
			newTagInput = '';
		}
	}

	function removeTag(tagName: string) {
		selectedTags = selectedTags.filter((t) => t !== tagName);
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-[400px] max-w-lg">
		<div class="mb-4 flex items-center justify-between">
			<h3 class="h3 flex items-center gap-2">
				<Tag class="h-5 w-5" />
				{title}
			</h3>
		</div>

		{#if message}
			<p class="mb-4 text-sm">{message}</p>
		{/if}

		<div class="space-y-4">
			<!-- Show pal count -->
			<p class="text-surface-600 dark:text-surface-400 text-sm">
				Editing tags for {pals.length} selected pal{pals.length > 1 ? 's' : ''}
			</p>

			<!-- Available Tags -->
			{#if upsState.availableTags.length > 0}
				<div>
					<span class="mb-2 block text-sm font-medium">Available Tags</span>
					<div class="flex flex-wrap gap-2">
						{#each upsState.availableTags as tag}
							<button
								type="button"
								onclick={() => toggleTag(tag.name)}
								class="rounded border px-2 py-1 text-xs transition-colors"
								class:bg-primary-500={selectedTags.includes(tag.name)}
								class:text-white={selectedTags.includes(tag.name)}
								class:border-primary-500={selectedTags.includes(tag.name)}
								class:hover:bg-surface-200={!selectedTags.includes(tag.name)}
								class:dark:hover:bg-surface-700={!selectedTags.includes(tag.name)}
							>
								{tag.name}
							</button>
						{/each}
					</div>
				</div>
			{/if}

			<!-- Add New Tag -->
			<div>
				<span class="mb-2 block text-sm font-medium">Add New Tag</span>
				<div class="flex items-center gap-2">
					<Input
						type="text"
						bind:value={newTagInput}
						placeholder="Enter tag name"
						inputClass="flex-1"
						onkeydown={(e: KeyboardEvent) => e.key === 'Enter' && addNewTag()}
					/>
					<TooltipButton
						popupLabel="Add Tag"
						onclick={addNewTag}
						buttonClass="bg-primary-500 hover:bg-primary-600"
					>
						<Plus class="h-4 w-4" />
					</TooltipButton>
				</div>
			</div>

			<!-- Selected Tags Display -->
			{#if selectedTags.length > 0}
				<div>
					<span class="mb-2 block text-sm font-medium">Selected Tags</span>
					<div class="flex flex-wrap gap-1">
						{#each selectedTags as tag}
							<span
								class="bg-primary-500 flex items-center gap-1 rounded px-2 py-1 text-xs text-white"
							>
								{tag}
								<button
									onclick={() => removeTag(tag)}
									class="hover:bg-primary-600 rounded"
									type="button"
								>
									<X class="h-3 w-3" />
								</button>
							</span>
						{/each}
					</div>
				</div>
			{/if}

			<!-- Show tags that will be added/removed -->
			{#if existingTags.length > 0}
				{@const tagsToAdd = selectedTags.filter((tag) => !existingTags.includes(tag))}
				{@const tagsToRemove = existingTags.filter((tag) => !selectedTags.includes(tag))}
				<div class="bg-surface-100 dark:bg-surface-800 rounded p-3 text-sm">
					<p class="mb-2 font-medium">Tag Changes:</p>

					{#if tagsToAdd.length > 0}
						<p class="text-green-600 dark:text-green-400">
							+ Add: {tagsToAdd.join(', ')}
						</p>
					{/if}
					{#if tagsToRemove.length > 0}
						<p class="text-red-600 dark:text-red-400">
							- Remove: {tagsToRemove.join(', ')}
						</p>
					{/if}
					{#if tagsToAdd.length === 0 && tagsToRemove.length === 0}
						<p class="text-surface-500">No changes</p>
					{/if}
				</div>
			{/if}
		</div>

		<!-- Actions -->
		<div class="mt-6 flex justify-end gap-2">
			<button
				type="button"
				onclick={() => handleClose(false)}
				class="bg-surface-500 hover:bg-surface-600 flex items-center gap-2 rounded-md px-4 py-2 text-white"
			>
				<X class="h-4 w-4" />
				Cancel
			</button>
			<button
				type="button"
				onclick={() => handleClose(true)}
				class="bg-primary-500 hover:bg-primary-600 flex items-center gap-2 rounded-md px-4 py-2 text-white"
				data-modal-primary
			>
				<Save class="h-4 w-4" />
				Save Changes
			</button>
		</div>
	</Card>
</div>
