<script lang="ts">
	import { Card, Combobox, Input, TooltipButton } from '$components/ui';
	import { Save, X, Folder, Plus } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import { getUpsState } from '$states';
	import type { SelectOption, UPSPal } from '$types';
	import * as m from '$i18n/messages';
	import { c, p } from '$lib/utils/commonTranslations';

	let {
		title = m.add_to_collection(),
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
	let selectedCollectionId: number | undefined = $state(undefined);
	let isCreatingCollection = $state(false);
	let newCollectionName = $state('');
	let newCollectionDescription = $state('');

	// Get existing collection assignments
	const existingCollections = $derived.by(() => {
		const collections = new Set<number>();
		pals.forEach((pal) => {
			if (pal.collection_id) {
				collections.add(pal.collection_id);
			}
		});
		return Array.from(collections);
	});

	// Check if all selected pals have the same collection
	const commonCollection = $derived.by(() => {
		if (existingCollections.length === 1) {
			return existingCollections[0];
		}
		return undefined;
	});

	// Initialize with common collection if exists
	$effect(() => {
		if (commonCollection) {
			selectedCollectionId = commonCollection;
		}
	});

	// Collection options for combobox
	const collectionOptions: SelectOption[] = $derived(
		upsState.filteredCollections.map((c) => ({
			label: c.name,
			value: c.id.toString()
		}))
	);

	function handleClose(confirmed: boolean) {
		if (!confirmed) {
			closeModal(null);
			return;
		}

		closeModal({
			collectionId: selectedCollectionId,
			removeFromCollection: selectedCollectionId === undefined
		});
	}

	async function createCollection() {
		if (!newCollectionName.trim()) return;

		await upsState.createCollection(newCollectionName.trim(), newCollectionDescription.trim());

		// Find the newly created collection
		const newCollection = upsState.collections.find((c) => c.name === newCollectionName.trim());
		if (newCollection) {
			selectedCollectionId = newCollection.id;
		}

		isCreatingCollection = false;
		newCollectionName = '';
		newCollectionDescription = '';
	}

	function cancelCreateCollection() {
		isCreatingCollection = false;
		newCollectionName = '';
		newCollectionDescription = '';
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-[400px] max-w-lg">
		<div class="mb-4 flex items-center justify-between">
			<h3 class="h3 flex items-center gap-2">
				<Folder class="h-5 w-5" />
				{title}
			</h3>
		</div>

		{#if message}
			<p class="mb-4 text-sm">{message}</p>
		{/if}

		<div class="space-y-4">
			<!-- Show pal count -->
			<p class="text-surface-600 dark:text-surface-400 text-sm">
				{m.manage_collection_for_pals({ count: pals.length, pals: c.pals })}
			</p>

			<!-- Show current collection status -->
			{#if existingCollections.length > 0}
				<div class="bg-surface-100 dark:bg-surface-800 rounded p-3 text-sm">
					{#if commonCollection}
						{@const collection = upsState.collections.find((c) => c.id === commonCollection)}
						<p class="mb-1 font-medium">{m.current_collection()}:</p>
						<p class="text-surface-600 dark:text-surface-400">
							{collection ? collection.name : m.unknown_collection()}
						</p>
					{:else}
						<p class="mb-1 font-medium">{m.mixed_collections()}:</p>
						<p class="text-surface-600 dark:text-surface-400">
							{m.mixed_collections_message({ pals: c.pals })}
						</p>
					{/if}
				</div>
			{/if}

			<!-- Collection Selection -->
			<div>
				<div class="mb-2 flex items-center justify-between">
					<span class="text-sm font-medium">{m.collection({ count: 1 })}</span>
					{#if !isCreatingCollection}
						<TooltipButton
							popupLabel={m.create_new_collection()}
							onclick={() => (isCreatingCollection = true)}
							buttonClass="bg-primary-500 hover:bg-primary-600"
						>
							<Plus class="h-4 w-4" />
						</TooltipButton>
					{/if}
				</div>

				{#if !isCreatingCollection}
					<div class="space-y-2">
						<Combobox
							bind:value={selectedCollectionId}
							options={collectionOptions}
							placeholder={m.select_entity({ entity: m.collection({ count: 1 }) })}
							inputClass="w-full"
						/>
						<button
							type="button"
							onclick={() => (selectedCollectionId = undefined)}
							class="text-sm text-red-500 hover:text-red-600"
						>
							{m.remove_from_collection()}
						</button>
					</div>
				{:else}
					<div class="space-y-2">
						<Input
							type="text"
							bind:value={newCollectionName}
							placeholder={m.collection_name()}
							inputClass="w-full"
						/>
						<Input
							type="text"
							bind:value={newCollectionDescription}
							placeholder={m.description_optional()}
							inputClass="w-full"
						/>
						<div class="flex gap-2">
							<button
								type="button"
								onclick={createCollection}
								class="rounded bg-green-500 px-3 py-1 text-sm text-white hover:bg-green-600"
								disabled={!newCollectionName.trim()}
							>
								{m.create()}
							</button>
							<button
								type="button"
								onclick={cancelCreateCollection}
								class="rounded bg-gray-500 px-3 py-1 text-sm text-white hover:bg-gray-600"
							>
								{m.cancel()}
							</button>
						</div>
					</div>
				{/if}
			</div>

			<!-- Show what will happen -->
			{#if selectedCollectionId !== commonCollection}
				<div class="bg-surface-100 dark:bg-surface-800 rounded p-3 text-sm">
					<p class="mb-2 font-medium">{m.changes()}:</p>
					{#if selectedCollectionId === undefined}
						<p class="text-red-600 dark:text-red-400">
							{m.remove_all_from_collections(p.pals)}
						</p>
					{:else}
						{@const targetCollection = upsState.collections.find(
							(c) => c.id === selectedCollectionId
						)}
						<p class="text-green-600 dark:text-green-400">
							{m.move_all_to_collection({ pals: c.pals, name: targetCollection?.name || m.unknown() })}
						</p>
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
				{m.cancel()}
			</button>
			<button
				type="button"
				onclick={() => handleClose(true)}
				class="bg-primary-500 hover:bg-primary-600 flex items-center gap-2 rounded-md px-4 py-2 text-white"
				data-modal-primary
				disabled={isCreatingCollection}
			>
				<Save class="h-4 w-4" />
				{selectedCollectionId === undefined ? m.remove() : m.move()}
				{m.to_collection()}
			</button>
		</div>
	</Card>
</div>