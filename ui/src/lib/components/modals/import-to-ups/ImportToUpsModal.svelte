<script lang="ts">
	import { Card, Combobox, Input, List, TooltipButton } from '$components/ui';
	import { Save, X, Folder, Tag, FileText, Plus, Trash, ReplaceAll } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import { getAppState, getUpsState } from '$states';
	import type { ImportToUpsModalResults, Pal, Player } from '$types';

	let {
		title = 'Save File 🡆 UPS',
		message = '',
		closeModal
	}: {
		title?: string;
		message?: string;
		closeModal: (value: any) => void;
	} = $props();

	const upsState = getUpsState();
	const appState = getAppState();

	let modalContainer: HTMLDivElement;
	let sourceType: 'pal_box' | 'gps' | 'dps' = $state('pal_box');
	let selectedCollectionId: number | undefined = $state(undefined);
	let selectedTags: string[] = $state([]);
	let notes: string = $state('');
	let newTagInput: string = $state('');
	let isCreatingCollection = $state(false);
	let newCollectionName = $state('');
	let newCollectionDescription = $state('');
	let selectedPlayerId: string | undefined = $state(undefined);
	let selectedPals: string[] = $state([]);
	let selectedPalId: string | undefined = $state(undefined);
	let result: ImportToUpsModalResults[] = $state([]);

	const gridClass = $derived.by(() => {
		if (selectedPlayerId) {
			return 'grid-cols-2';
		}
		return 'grid-cols-1';
	});

	const palOptions = $derived.by(() => {
		if (selectedPlayerId) {
			switch (sourceType) {
				case 'pal_box':
					return (
						Object.entries(appState.players[selectedPlayerId]?.pals || {})
							.filter(([id]) => !selectedPals.includes(id))
							.map(([id, pal]) => ({
								label: pal.nickname || pal.character_key || pal.character_id,
								value: id
							})) || []
					);
				case 'gps':
					return (
						Object.entries(appState.gps || {})
							.filter(([id]) => !selectedPals.includes(id))
							.map(([id, pal]) => ({
								label: pal.nickname || pal.character_key || pal.character_id,
								value: id
							})) || []
					);
				case 'dps':
					return (
						Object.entries(appState.players[selectedPlayerId]?.dps || {})
							.filter(([id]) => !selectedPals.includes(id))
							.map(([id, pal]) => ({
								label: pal.nickname || pal.character_key || pal.character_id,
								value: id
							})) || []
					);
			}
		}
		return [];
	});

	const pals = $derived.by(() => {
		let pals: Pal[] = [];
		if (selectedPals.length > 0 && selectedPlayerId) {
			pals = Object.values(appState.players[selectedPlayerId]?.pals || {}).filter((pal) =>
				selectedPals.includes(pal.instance_id)
			);
			pals.push(
				...Object.values(appState.players[selectedPlayerId]?.dps || {}).filter((pal) =>
					selectedPals.includes(pal.instance_id)
				)
			);
		}
		pals.push(
			...Object.values(appState.gps || {}).filter((pal) => selectedPals.includes(pal.instance_id))
		);
		return pals;
	});

	function handleClose(confirmed: boolean) {
		if (!confirmed) {
			closeModal(null);
			return;
		}

		closeModal(result);
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

	function addPal(palId?: string) {
		if (!palId || !selectedPlayerId) return;
		let pal: Pal | undefined;
		switch (sourceType) {
			case 'pal_box':
				pal = appState.players[selectedPlayerId]?.pals?.[palId];
				break;
			case 'gps':
				pal = Object.values(appState.gps || {}).find((p) => p.instance_id === palId);
				break;
			case 'dps':
				pal = Object.values(appState.players[selectedPlayerId]?.dps || {}).find(
					(p) => p.instance_id === palId
				);
				break;
		}
		if (!pal) return;

		result.push({
			sourceType,
			palId,
			collectionId: selectedCollectionId,
			tags: selectedTags,
			notes,
			playerId: selectedPlayerId,
			sourceSlot: pal.storage_slot
		});
		selectedPals = [...selectedPals, palId];
		selectedPalId = undefined;
	}

	function addAllPals() {
		if (!selectedPlayerId) return;
		let pals: Pal[] | undefined = undefined;
		switch (sourceType) {
			case 'pal_box':
				pals = Object.values(appState.players[selectedPlayerId]?.pals || {});
				break;
			case 'gps':
				pals = Object.values(appState.gps || {});
				break;
			case 'dps':
				pals = Object.values(appState.players[selectedPlayerId]?.dps || {});
				break;
		}
		if (!pals) return;
		for (const pal of pals) {
			if (selectedPals.includes(pal.instance_id)) continue;
			result.push({
				sourceType,
				palId: pal.instance_id,
				collectionId: selectedCollectionId,
				tags: selectedTags,
				notes,
				playerId: selectedPlayerId,
				sourceSlot: pal.storage_slot
			});
			selectedPals = [...selectedPals, pal.instance_id];
		}
		selectedPalId = undefined;
	}

	function removePal(palId: string) {
		result = result.filter((r) => r.palId !== palId);
		selectedPals = selectedPals.filter((id) => id !== palId);
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
	<Card class="min-w-[calc(100vw/2)] max-w-2xl">
		<div class="mb-4 flex items-center justify-between">
			<h3 class="h3 flex items-center gap-2">
				<FileText class="h-5 w-5" />
				{title}
			</h3>
		</div>

		{#if message}
			<p class="mb-4 text-sm">{message}</p>
		{/if}

		<div class="grid {gridClass} gap-2">
			<div class="flex w-full flex-col space-y-4">
				<!-- Source Type Selection -->
				<div>
					<Combobox
						bind:value={selectedPlayerId}
						options={Object.entries(appState.players).map(([id, player]) => ({
							label: (player as Player).nickname || `Player ${id}`,
							value: id
						}))}
						placeholder="Select Player"
						inputClass="w-full"
						onChange={() => (selectedPals = [])}
					/>
					{#if selectedPlayerId}
						<span class="mb-2 block text-sm font-medium">Import From</span>
						<div class="grid grid-cols-3 gap-2">
							<label
								class="hover:bg-surface-100 dark:hover:bg-surface-800 border-surface-700 flex cursor-pointer items-center space-x-2 rounded border p-2"
								class:bg-primary-500={sourceType === 'pal_box'}
							>
								<input type="radio" bind:group={sourceType} value="pal_box" class="sr-only" />
								<span class="text-sm">Pal Box</span>
							</label>
							<label
								class="hover:bg-surface-100 dark:hover:bg-surface-800 border-surface-700 flex cursor-pointer items-center space-x-2 rounded border p-2"
								class:bg-primary-500={sourceType === 'gps'}
							>
								<input type="radio" bind:group={sourceType} value="gps" class="sr-only" />
								<span class="text-sm">GPS</span>
							</label>
							<label
								class="hover:bg-surface-100 dark:hover:bg-surface-800 border-surface-700 flex cursor-pointer items-center space-x-2 rounded border p-2"
								class:bg-primary-500={sourceType === 'dps'}
							>
								<input type="radio" bind:group={sourceType} value="dps" class="sr-only" />
								<span class="text-sm">DPS</span>
							</label>
						</div>
					{/if}
				</div>

				<!-- Collection Selection -->
				<div>
					<div class="flex items-center gap-2">
						<label class="flex items-center text-sm font-medium">
							<Folder class="mr-2 h-4 w-4" />
							Collection (Optional)
						</label>
						<TooltipButton
							popupLabel="Create New Collection"
							onclick={() => (isCreatingCollection = true)}
							buttonClass="bg-primary-500 hover:bg-primary-800"
						>
							<Plus class="h-4 w-4" />
						</TooltipButton>
					</div>
					{#if !isCreatingCollection}
						<div>
							<Combobox
								bind:value={selectedCollectionId}
								options={upsState.filteredCollections.map((c) => ({
									label: c.name,
									value: `${c.id}`
								}))}
								optionValue="id"
								optionLabel="name"
								placeholder="No Collection"
								inputClass="flex-1"
							/>
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
					<label class="mb-2 flex items-center gap-2 text-sm font-medium">
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
					{/if}

					<!-- Add New Tag -->
					<div class="flex items-center gap-2">
						<Input
							type="text"
							bind:value={newTagInput}
							placeholder="Add new tag"
							inputClass="flex-1"
							onkeydown={(e: KeyboardEvent) => e.key === 'Enter' && addNewTag()}
						/>
						<TooltipButton
							popupLabel="Add Tag"
							onclick={addNewTag}
							buttonClass="bg-primary-500 hover:bg-primary-800"
						>
							<Plus class="h-4 w-4" />
						</TooltipButton>
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
						placeholder="Add notes about these imported Pals..."
						rows="3"
						class="focus:outline-hidden ring-surface-200-800 focus-within:ring-secondary-500 rounded-xs resize-vertical border-surface-700 bg-surface-900 w-full border p-2 ring"
					></textarea>
				</div>
			</div>
			{#if selectedPlayerId}
				<div class="flex h-full flex-col">
					<div class="flex items-center gap-2">
						<Combobox options={palOptions} bind:value={selectedPalId} placeholder="Select Pal">
							{#snippet selectOption(option)}
								<span class="truncate">{option.label}</span>
							{/snippet}
						</Combobox>
						<TooltipButton
							popupLabel="Transfer Pal"
							onclick={() => addPal(selectedPalId)}
							buttonClass="bg-primary-500 hover:bg-primary-800"
						>
							<Plus class="h-4 w-4" />
						</TooltipButton>
						<TooltipButton
							popupLabel="Transfer all Pals"
							onclick={() => addAllPals()}
							buttonClass="bg-primary-500 hover:bg-primary-800"
						>
							<ReplaceAll class="h-4 w-4" />
						</TooltipButton>
					</div>
					<List items={pals} idKey="instance_id" baseClass="max-h-[435px]">
						{#snippet listItem(pal)}
							{pal.nickname || pal.character_key || pal.character_id}
						{/snippet}
						{#snippet listItemActions(pal)}
							<TooltipButton
								popupLabel="Remove Pal"
								onclick={() => removePal(pal.instance_id)}
								buttonClass="hover:bg-red-600"
							>
								<Trash class="h-4 w-4" />
							</TooltipButton>
						{/snippet}
						{#snippet listItemPopup(pal)}
							{pal.nickname || pal.character_key || pal.character_id}
						{/snippet}
					</List>
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
				Import
			</button>
		</div>
	</Card>
</div>
