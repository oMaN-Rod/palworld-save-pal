<script lang="ts">
	import { untrack, type Component } from 'svelte';
	import { Button, Card, Checkbox, Input, Select, Spinner } from '$components/ui';
	import { getModalState, getModsState, getNexusState } from '$states';
	import { MessageType } from '$types';
	import type { NexusSort } from '$types';
	import * as m from '$i18n/messages';
	import { debounce } from '$lib/utils/debounce';
	import NexusAccountCard from './NexusAccountCard.svelte';
	import NexusDecisionsModal from './NexusDecisionsModal.svelte';
	import NexusDetailDrawer from './NexusDetailDrawer.svelte';
	import NexusModCard from './NexusModCard.svelte';
	import { nexusErrorText } from './nexusText';

	let { targetId }: { targetId: string } = $props();

	const modsState = getModsState();
	const nexusState = getNexusState();
	const modal = getModalState();

	let selectedId = $state<number | null>(null);
	let queryText = $state(nexusState.query);

	const refusal = $derived(modsState.lastErrorFor(MessageType.NEXUS_SEARCH));
	const searching = $derived(nexusState.isBusy('searching'));

	const sorts: { value: NexusSort; label: () => string }[] = [
		{ value: 'relevance', label: m.mods_discover_sort_relevance },
		{ value: 'downloads', label: m.mods_discover_sort_downloads },
		{ value: 'endorsements', label: m.mods_discover_sort_endorsements },
		{ value: 'updated', label: m.mods_discover_sort_updated },
		{ value: 'created', label: m.mods_discover_sort_created },
		{ value: 'name', label: m.mods_discover_sort_name }
	];

	const sortOptions = $derived(
		sorts.map((entry) => ({ value: entry.value, label: entry.label() }))
	);

	const categoryOptions = $derived([
		{ value: '', label: m.mods_discover_category_all() },
		...nexusState.categories.map((category) => ({
			value: String(category.category_id),
			label: category.name
		}))
	]);

	/** The store's own default when `sort` is unset: matches what `search()` actually sends. */
	const sortDefault = $derived(nexusState.query.trim() ? 'relevance' : 'downloads');

	function installedModId(modId: number): boolean {
		return modsState.mods.some((entry) => entry.nexus_mod_id === modId);
	}

	$effect(() => {
		targetId;
		modsState.resets;
		untrack(() => {
			if (nexusState.categories.length === 0) nexusState.loadCategories();
			if (!nexusState.searched) nexusState.search();
		});
	});

	$effect(() => {
		const pending = nexusState.pendingDecisions;
		if (!pending) return;
		untrack(() => {
			modal.showModal(NexusDecisionsModal as unknown as Component, { reply: pending });
		});
	});

	function runSearch(): void {
		nexusState.query = queryText;
		nexusState.search();
	}

	const debouncedSearch = debounce(runSearch, 300);

	function selectCategory(value: string | number): void {
		const next = value === '' ? null : String(value);
		if (next === nexusState.category) return;
		nexusState.category = next;
		nexusState.search();
	}

	function selectSort(value: string | number): void {
		const next = value as NexusSort;
		if (next === (nexusState.sort ?? sortDefault)) return;
		nexusState.sort = next;
		nexusState.search();
	}

	function toggleAdult(): void {
		nexusState.includeAdult = !nexusState.includeAdult;
		nexusState.search();
	}
</script>

<div class="flex flex-col gap-4">
	<details>
		<summary>NexusMods</summary>
		<NexusAccountCard />
	</details>
	<div
		class={['grid items-start gap-4', selectedId !== null && 'lg:grid-cols-[minmax(0,1fr)_20rem]']}
	>
		<div class="flex flex-col gap-3">
			<div class="flex flex-wrap items-end gap-3">
				<div class="min-w-48 flex-1">
					<Input
						type="search"
						label={m.mods_discover_search_label()}
						placeholder={m.mods_discover_search_placeholder()}
						bind:value={queryText}
						inputClass="w-full"
						oninput={debouncedSearch}
					/>
				</div>
				<div class="w-48">
					<Select
						label={m.mods_discover_category_label()}
						options={categoryOptions}
						value={nexusState.category ?? ''}
						onChange={selectCategory}
					/>
				</div>
				<div class="w-40">
					<Select
						label={m.mods_discover_sort_label()}
						options={sortOptions}
						value={nexusState.sort ?? sortDefault}
						onChange={selectSort}
					/>
				</div>
				<Checkbox
					label={m.mods_discover_adult()}
					checked={nexusState.includeAdult}
					onchange={toggleAdult}
				/>
			</div>

			{#if refusal}
				<p class="text-error-400 text-sm" role="alert">{nexusErrorText(refusal)}</p>
			{/if}

			{#if searching && nexusState.results.length === 0}
				<Card padding="p-6" class="text-surface-400 flex items-center gap-3 text-sm">
					<Spinner size="size-4" />
				</Card>
			{:else if nexusState.results.length === 0}
				<p class="text-surface-400 py-4 text-center text-sm">
					{nexusState.searched ? m.mods_discover_empty() : m.mods_discover_start()}
				</p>
			{:else}
				<div
					class={[
						'grid gap-3',
						selectedId === null
							? 'grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5'
							: 'grid-cols-1 md:grid-cols-2 xl:grid-cols-3'
					]}
				>
					{#each nexusState.results as mod (mod.mod_id)}
						<NexusModCard
							{mod}
							selected={mod.mod_id === selectedId}
							installed={installedModId(mod.mod_id)}
							onDetails={(modId) => (selectedId = modId)}
						/>
					{/each}
				</div>
				<p class="text-surface-400 text-center text-xs">
					{m.mods_discover_results({
						shown: nexusState.results.length,
						total: nexusState.totalCount
					})}
				</p>
				{#if nexusState.hasMore}
					<Button
						variant="secondary"
						size="sm"
						class="mx-auto"
						disabled={searching}
						onclick={() => nexusState.loadMore()}
					>
						{m.mods_discover_load_more()}
					</Button>
				{/if}
			{/if}
		</div>
		{#if selectedId !== null}
			<div class="order-first lg:order-none">
				<NexusDetailDrawer {targetId} modId={selectedId} onClose={() => (selectedId = null)} />
			</div>
		{/if}
	</div>
</div>
