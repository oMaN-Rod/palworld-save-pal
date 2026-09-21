<script lang="ts" generics="TPal, TId extends string | number">
	import { untrack, type Snippet } from 'svelte';

	import { longPress } from '$lib/actions/longPress';
	import PalGrid from '$components/pal/PalGrid.svelte';
	import type { ActionDescriptor } from '$components/ui/actions';
	import ActionSheet from '$components/ui/sheet/ActionSheet.svelte';
	import {
		createPalContainerState,
		pageCountOf,
		type PalContainerSelection,
		type PalContainerServerPaging
	} from '$states/palContainer.svelte';
	import { layout } from '$utils/layout.svelte';
	import * as m from '$i18n/messages';

	import PalContainerToolbar from './PalContainerToolbar.svelte';
	import PalDetailSheet from './PalDetailSheet.svelte';
	import PalFilterSheet from './PalFilterSheet.svelte';
	import PalList from './PalList.svelte';
	import PalPager from './PalPager.svelte';

	/**
	 * Paged, filterable, selectable pal container. Given `serverPaging` or
	 * `selection`, the caller owns that state and the view only reports changes.
	 */
	let {
		pals,
		idOf,
		nicknameOf,
		storageKey,
		title,
		pageSize = 30,
		actions = [],
		matches,
		serverPaging,
		selection,
		portrait,
		columns,
		filters,
		detail,
		palActions,
		onOpenPal
	}: {
		pals: TPal[];
		idOf: (pal: TPal) => TId;
		nicknameOf: (pal: TPal) => string;
		/** Suffix for the persisted view-mode key; unique per consumer page. */
		storageKey: string;
		title: string;
		/** Client-paged size. Ignored when `serverPaging` carries the server's. */
		pageSize?: number;
		actions?: ActionDescriptor[];
		matches?: (pal: TPal, criteria: { query: string; filter: string }) => boolean;
		/** Set when the caller pages on the server; `pals` is then one page. */
		serverPaging?: PalContainerServerPaging;
		/** Set when the caller owns the selection, as its bulk actions need it. */
		selection?: PalContainerSelection<TId>;
		portrait: Snippet<[TPal]>;
		/** Must render non-interactive content. */
		columns: Snippet<[TPal]>;
		filters?: Snippet<[{ selected: string; select: (value: string) => void }]>;
		/** Body of the detail sheet a tap opens on a phone. */
		detail?: Snippet<[TPal]>;
		palActions?: (pal: TPal) => ActionDescriptor[];
		/** Where a tap goes when there is room for a real editor. */
		onOpenPal?: (pal: TPal) => void;
	} = $props();

	let searchQuery = $state('');
	let selectedFilter = $state('All');
	let filterOpen = $state(false);
	let detailPal = $state<TPal | null>(null);
	let actionPal = $state<TPal | null>(null);

	const filtered = $derived(
		matches
			? pals.filter((pal) => matches(pal, { query: searchQuery, filter: selectedFilter }))
			: pals
	);

	// Built once from mount-time props, with paging disabled when the caller pages.
	// svelte-ignore state_referenced_locally
	const container = createPalContainerState<TPal, TId>(
		serverPaging
			? { key: storageKey, idOf }
			: { key: storageKey, idOf, pageSize, totalOf: () => filtered.length }
	);

	const pageCount = $derived(
		serverPaging
			? pageCountOf(serverPaging.totalCount, serverPaging.pageSize)
			: container.pageCount()
	);
	const page = $derived(serverPaging ? serverPaging.page : container.page);
	const pagePals = $derived.by(() => {
		// Already sliced by the server.
		if (serverPaging) return filtered;
		// `container.pageSize` so the slice matches the pager's count.
		const size = container.pageSize ?? filtered.length;
		return filtered.slice((page - 1) * size, page * size);
	});

	// A function binding would do, but svelte-check cannot type one.
	const pageBinding = {
		get page(): number {
			return page;
		},
		set page(next: number) {
			if (serverPaging) {
				serverPaging.onPageChange(next);
				return;
			}
			container.setPage(next);
		}
	};

	const selected = {
		get ids(): TId[] {
			return selection ? Array.from(selection.ids) : container.selection;
		},
		get count(): number {
			return selection ? selection.ids.size : container.selection.length;
		},
		has(id: TId): boolean {
			return selection ? selection.ids.has(id) : container.isSelected(id);
		},
		toggle(id: TId): void {
			if (selection) {
				selection.onToggle(id);
				return;
			}
			container.toggleSelected(id);
		}
	};

	// New criteria restart at page 1; a server-paging caller resets its own page.
	let lastCriteria: string | undefined;

	$effect(() => {
		const criteria = `${selectedFilter}\u0000${searchQuery}`;
		if (criteria === lastCriteria) return;
		lastCriteria = criteria;
		// Untracked so this does not also subscribe to `pals`.
		untrack(() => container.setPage(1));
	});

	const filterable = $derived(Boolean(matches || filters));

	const detailActions = $derived(detailPal && palActions ? palActions(detailPal) : []);

	// Select leads because a long press is the only route to selection on touch.
	const actionRows = $derived.by((): ActionDescriptor[] => {
		const pal = actionPal;
		if (!pal) return [];
		const isSelected = selected.has(idOf(pal));
		return [
			{
				id: 'pal-container-select',
				label: isSelected ? m.deselect() : m.select(),
				icon: isSelected ? 'tabler:square-off' : 'tabler:square-check',
				run: () => toggleSelected(pal)
			},
			...(palActions?.(pal) ?? [])
		];
	});

	function handleSelect(pal: TPal): void {
		if (layout.phone) {
			detailPal = pal;
			return;
		}
		onOpenPal?.(pal);
	}

	function toggleSelected(pal: TPal): void {
		selected.toggle(idOf(pal));
	}

	function handleLongPress(pal: TPal): void {
		actionPal = pal;
	}

	// A plain click opens the badge's editor; stopped so it never also does on Ctrl-click.
	function handleGridActivate(event: MouseEvent, pal: TPal): void {
		if (!event.ctrlKey && !event.metaKey) return;
		event.preventDefault();
		event.stopPropagation();
		toggleSelected(pal);
	}

	// Browsers synthesize `contextmenu` from a long press, which would open the badge's menu.
	function handleContextMenu(event: MouseEvent): void {
		if (!layout.coarse) return;
		event.preventDefault();
		event.stopPropagation();
	}
</script>

{#snippet filterPanel()}
	<div class="flex flex-col gap-4" data-testid="pal-container-filters">
		{#if matches}
			<label class="label">
				<span class="label-text">{m.search()}</span>
				<input
					class="input"
					type="search"
					bind:value={searchQuery}
					placeholder={m.search_by_name_nickname()}
				/>
			</label>
		{/if}

		{#if filters}
			{@render filters({
				selected: selectedFilter,
				select: (value: string) => (selectedFilter = value)
			})}
		{/if}
	</div>
{/snippet}

<div class="flex min-h-0 flex-col gap-2">
	<PalContainerToolbar
		bind:viewMode={container.viewMode}
		selectionCount={selected.count}
		{actions}
		{filterable}
		onOpenFilter={() => (filterOpen = !filterOpen)}
		{title}
	/>

	{#if filterOpen && !layout.phone}
		{@render filterPanel()}
	{/if}

	<div class="min-h-0 flex-1 overflow-y-auto">
		{#if container.viewMode === 'list'}
			<PalList
				pals={pagePals}
				{idOf}
				{nicknameOf}
				selectedIds={selected.ids}
				onSelect={handleSelect}
				onLongPress={handleLongPress}
				{portrait}
				{columns}
			/>
		{:else}
			<PalGrid data-testid="pal-container-grid">
				{#each pagePals as pal (idOf(pal))}
					<div
						role="presentation"
						data-testid="pal-container-badge"
						oncontextmenucapture={handleContextMenu}
						onclickcapture={(event) => handleGridActivate(event, pal)}
						use:longPress={{ onLongPress: () => handleLongPress(pal) }}
					>
						{@render portrait(pal)}
					</div>
				{/each}
			</PalGrid>
		{/if}
	</div>

	{#if pageCount > 1}
		<PalPager
			bind:page={pageBinding.page}
			{pageCount}
			label={m.page_of_pages({ current: page, total: pageCount })}
		/>
	{/if}
</div>

{#if layout.phone}
	<PalFilterSheet open={filterOpen} onClose={() => (filterOpen = false)}>
		{@render filterPanel()}
	</PalFilterSheet>
{/if}

{#if actionPal}
	{@const pal = actionPal}
	<ActionSheet
		open
		title={nicknameOf(pal)}
		actions={actionRows}
		onClose={() => (actionPal = null)}
	/>
{/if}

{#if detailPal}
	{@const pal = detailPal}
	<PalDetailSheet
		open
		title={nicknameOf(pal)}
		actions={detailActions}
		onClose={() => (detailPal = null)}
	>
		{#if detail}
			{@render detail(pal)}
		{/if}
	</PalDetailSheet>
{/if}
