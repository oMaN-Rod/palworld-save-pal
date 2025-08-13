<script lang="ts">
	import { Card, Combobox, Input } from '$components/ui';
	import { X, Folder, Tag, Plus, Copy } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import { getUpsState } from '$states';
	import type { Pal, SelectOption } from '$types';

	let {
		title = 'Clone to UPS',
		message = '',
		pals = [],
		closeModal
	} = $props<{
		title?: string;
		message?: string;
		pals: Pal[];
		closeModal: (value: any) => void;
	}>();

	const upsState = getUpsState();

	let modalContainer: HTMLDivElement;
	let selectedCollectionId: number | undefined = $state(undefined);
	let selectedTags: string[] = $state([]);
	let notes: string = $state('');
	let newTagInput: string = $state('');
	let isCreatingCollection = $state(false);
	let newCollectionName = $state('');
	let newCollectionDescription = $state('');

	function handleClose(confirmed: boolean) {
		if (!confirmed) {
			closeModal(null);
			return;
		}

		closeModal({
			collectionId: selectedCollectionId,
			tags: selectedTags,
			notes
		});
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

	async function createCollection() {
		if (!newCollectionName.trim()) return;

		await upsState.createCollection(newCollectionName.trim(), newCollectionDescription.trim());

		const newCollection = upsState.collections.find((c) => c.name === newCollectionName.trim());
		if (newCollection) {
			selectedCollectionId = newCollection.id;
		}

		isCreatingCollection = false;
		newCollectionName = '';
		newCollectionDescription = '';
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-[calc(100vw/2)] max-w-lg">
		<div class="mb-4 flex items-center justify-between">
			<h3 class="h3 flex items-center gap-2">
				<Copy class="h-5 w-5" />
				{title}
			</h3>
		</div>

		{#if message}
			<p class="mb-4 text-sm">{message}</p>
		{/if}

		<!-- Pal Summary -->
		<div class="bg-surface-100 dark:bg-surface-800 mb-4 rounded-lg p-3">
			<h4 class="mb-2 text-sm font-medium">
				Cloning {pals.length} pal{pals.length > 1 ? 's' : ''}
			</h4>
			{#if pals.length <= 3}
				<div class="space-y-1">
					{#each pals as pal}
						<div class="text-surface-600 dark:text-surface-400 text-sm">
							• {pal.nickname || pal.name || pal.character_id} (Level {pal.level})
						</div>
					{/each}
				</div>
			{:else}
				<div class="text-surface-600 dark:text-surface-400 text-sm">
					• {pals[0].nickname || pals[0].name || pals[0].character_id} (Level {pals[0].level})
					<br />
					• {pals[1].nickname || pals[1].name || pals[1].character_id} (Level {pals[1].level})
					<br />
					• ...and {pals.length - 2} more
				</div>
			{/if}
		</div>

		<div class="space-y-4">
			<!-- Collection Selection -->
			<div>
				<label class="mb-2 block flex items-center gap-2 text-sm font-medium">
					<Folder class="h-4 w-4" />
					Collection (Optional)
				</label>
				{#if !isCreatingCollection}
					<div class="flex gap-2">
						<Combobox
							bind:value={selectedCollectionId}
							options={upsState.filteredCollections.map((c) => ({
								value: c.id,
								label: c.name
							})) as SelectOption[]}
							placeholder="Select a collection"
						/>

						<button
							type="button"
							onclick={() => (isCreatingCollection = true)}
							class="bg-primary-500 hover:bg-primary-600 flex w-10 items-center justify-center gap-1 rounded-md p-2 text-white"
						>
							<Plus class="h-4 w-4" />
						</button>
					</div>
				{:else}
					<div class="space-y-2">
						<Input
							type="text"
							bind:value={newCollectionName}
							placeholder="Collection name"
							inputClass="w-full"
						/>
						<Input
							type="text"
							bind:value={newCollectionDescription}
							placeholder="Description (optional)"
							inputClass="w-full"
						/>
						<div class="flex gap-2">
							<button
								type="button"
								onclick={createCollection}
								class="rounded bg-green-500 px-3 py-1 text-sm text-white hover:bg-green-600"
							>
								Create
							</button>
							<button
								type="button"
								onclick={() => {
									isCreatingCollection = false;
									newCollectionName = '';
									newCollectionDescription = '';
								}}
								class="rounded bg-gray-500 px-3 py-1 text-sm text-white hover:bg-gray-600"
							>
								Cancel
							</button>
						</div>
					</div>
				{/if}
			</div>

			<!-- Tags Selection -->
			<div>
				<label class="mb-2 block flex items-center gap-2 text-sm font-medium">
					<Tag class="h-4 w-4" />
					Tags (Optional)
				</label>

				<!-- Existing Tags -->
				{#if upsState.availableTags.length > 0}
					<div class="mb-2 flex flex-wrap gap-2">
						{#each upsState.availableTags as tag}
							<button
								type="button"
								onclick={() => toggleTag(tag.name)}
								class="border-surface-800 rounded border px-2 py-1 text-xs transition-colors"
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
				{/if}

				<!-- Add New Tag -->
				<div class="flex gap-2">
					<Input
						type="text"
						bind:value={newTagInput}
						placeholder="Add new tag"
						onkeydown={(e: KeyboardEvent) => e.key === 'Enter' && addNewTag()}
					/>
					<button
						type="button"
						onclick={addNewTag}
						class="bg-primary-500 hover:bg-secondary-600 focus:outline-hidden ring-surface-200-800 focus-within:ring-secondary-500 flex w-10 items-center justify-center gap-1 rounded-md px-3 py-2 text-white ring"
					>
						<Plus class="h-4 w-4" />
					</button>
				</div>

				<!-- Selected Tags Display -->
				{#if selectedTags.length > 0}
					<div class="mt-2">
						<span class="text-surface-600 dark:text-surface-400 text-sm">Selected:</span>
						<div class="mt-1 flex flex-wrap gap-1">
							{#each selectedTags as tag}
								<span
									class="bg-primary-500 flex items-center gap-1 rounded px-2 py-1 text-xs text-white"
								>
									{tag}
									<button onclick={() => toggleTag(tag)} class="hover:bg-primary-600 rounded">
										<X class="h-3 w-3" />
									</button>
								</span>
							{/each}
						</div>
					</div>
				{/if}
			</div>

			<!-- Notes -->
			<div>
				<label for="notes" class="mb-2 block text-sm font-medium">Notes (Optional)</label>
				<textarea
					id="notes"
					bind:value={notes}
					placeholder="Add notes about these cloned Pals..."
					rows="3"
					class="bg-surface-900 resize-vertical border-surface-800 focus:outline-hidden ring-surface-200-800 focus-within:ring-secondary-500 w-full rounded-md border p-2 ring"
				></textarea>
			</div>
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
				<Copy class="h-4 w-4" />
				Clone to UPS
			</button>
		</div>
	</Card>
</div>
