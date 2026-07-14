<script lang="ts" generics="T">
	import { cn } from '$theme';
	import { Checkbox } from '$components/ui';
	import { ChevronDown, ChevronUp, ChevronsUpDown } from 'lucide-svelte';
	import type { Snippet } from 'svelte';
	import type { ColumnDef, SortState } from './table.types';
	import {
		areAllSelected,
		computePageInfo,
		nextSortState,
		paginateRows,
		setPageSelection,
		sortRows,
		toggleSelection
	} from './table.utils';

	// Contract notes for downstream consumers:
	// - When `onrowclick` is set, data rows are keyboard-activatable (Enter/Space)
	//   in addition to being clickable with a pointer.
	// - `pageSize` is a fixed value owned by the PARENT/toolbar. Table has no built-in
	//   page-size selector by design; the toolbar renders one and passes the value down.
	// - Server-side mode (`serverSide: true`): changing the sort does NOT reset `page`
	//   to 1. A server-side consumer that wants to return to page 1 on sort must do so
	//   in its `onsort` handler.
	let {
		rows,
		columns,
		rowKey,
		selected = $bindable(new Set<string>()),
		selectable = true,
		sort = $bindable<SortState>({ key: null, direction: 'asc' }),
		page = $bindable(1),
		pageSize = 25,
		total,
		serverSide = false,
		cell,
		rowActions,
		empty,
		onsort = () => {},
		onpagechange = () => {},
		onrowclick
	}: {
		rows: T[];
		columns: ColumnDef<T>[];
		rowKey: (row: T) => string;
		selected?: Set<string>;
		selectable?: boolean;
		sort?: SortState;
		page?: number;
		pageSize?: number;
		total?: number;
		serverSide?: boolean;
		cell: Snippet<[{ row: T; column: ColumnDef<T> }]>;
		rowActions?: Snippet<[T]>;
		empty?: Snippet;
		onsort?: (sort: SortState) => void;
		onpagechange?: (page: number) => void;
		onrowclick?: (row: T) => void;
	} = $props();

	// In client mode the component sorts + paginates internally.
	// In server mode it renders `rows` as the already-fetched current page.
	const displayedRows = $derived(
		serverSide ? rows : paginateRows(sortRows(rows, columns, sort), { page, pageSize })
	);
	const totalCount = $derived(serverSide ? (total ?? rows.length) : rows.length);
	const pageInfo = $derived(computePageInfo({ page, pageSize }, totalCount));
	const pageKeys = $derived(displayedRows.map(rowKey));
	const allOnPageSelected = $derived(areAllSelected(selected, pageKeys));

	function handleHeaderClick(column: ColumnDef<T>) {
		if (!column.sortable) return;
		const updated = nextSortState(sort, column.key);
		sort = updated;
		if (!serverSide) {
			page = 1;
		}
		onsort(updated);
	}

	function handleSelectAll() {
		selected = setPageSelection(selected, pageKeys, !allOnPageSelected);
	}

	function handleRowCheckbox(row: T) {
		selected = toggleSelection(selected, rowKey(row));
	}

	function goToPage(target: number) {
		const clamped = computePageInfo({ page: target, pageSize }, totalCount).page;
		page = clamped;
		onpagechange(clamped);
	}

	function alignClass(column: ColumnDef<T>): string {
		if (column.align === 'center') return 'text-center';
		if (column.align === 'right') return 'text-right';
		return 'text-left';
	}

	function handleRowKeydown(event: KeyboardEvent, row: T) {
		if (!onrowclick) return;
		if (event.key === 'Enter') {
			onrowclick(row);
		} else if (event.key === ' ') {
			event.preventDefault();
			onrowclick(row);
		}
	}
</script>

<div class="flex flex-col gap-2">
	<div class="border-surface-900 overflow-x-auto border">
		<table class="w-full border-collapse text-sm">
			<thead class="bg-surface-900 sticky top-0 z-10">
				<tr>
					{#if selectable}
						<th scope="col" class="w-10 p-2">
							<Checkbox checked={allOnPageSelected} onchange={handleSelectAll} />
						</th>
					{/if}
					{#each columns as column (column.key)}
						<th
							scope="col"
							class={cn('p-2 font-semibold', alignClass(column), column.class)}
							aria-sort={column.sortable
								? sort.key === column.key
									? sort.direction === 'asc'
										? 'ascending'
										: 'descending'
									: 'none'
								: undefined}
						>
							{#if column.sortable}
								<button
									type="button"
									class="hover:text-prim-500 inline-flex items-center gap-1"
									onclick={() => handleHeaderClick(column)}
								>
									<span>{column.header}</span>
									{#if sort.key === column.key}
										{#if sort.direction === 'asc'}
											<ChevronUp class="h-4 w-4" />
										{:else}
											<ChevronDown class="h-4 w-4" />
										{/if}
									{:else}
										<ChevronsUpDown class="h-4 w-4 opacity-40" />
									{/if}
								</button>
							{:else}
								<span>{column.header}</span>
							{/if}
						</th>
					{/each}
					{#if rowActions}
						<th scope="col" class="w-px p-2 text-right">Actions</th>
					{/if}
				</tr>
			</thead>
			<tbody>
				{#each displayedRows as row (rowKey(row))}
					<tr
						class={cn(
							'border-surface-900 hover:bg-secondary-500/25 cursor-pointer border-t',
							selected.has(rowKey(row)) ? 'bg-secondary-500/25' : ''
						)}
						onclick={() => onrowclick?.(row)}
						tabindex={onrowclick ? 0 : undefined}
						role={onrowclick ? 'button' : undefined}
						onkeydown={onrowclick ? (event: KeyboardEvent) => handleRowKeydown(event, row) : undefined}
					>
						{#if selectable}
							<td class="p-2" onclick={(event) => event.stopPropagation()} onkeydown={(event) => event.stopPropagation()}>
								<Checkbox
									checked={selected.has(rowKey(row))}
									onchange={() => handleRowCheckbox(row)}
								/>
							</td>
						{/if}
						{#each columns as column (column.key)}
							<td class={cn('p-2', alignClass(column), column.class)}>
								{@render cell({ row, column })}
							</td>
						{/each}
						{#if rowActions}
							<td class="p-2 text-right" onclick={(event) => event.stopPropagation()} onkeydown={(event) => event.stopPropagation()}>
								{@render rowActions(row)}
							</td>
						{/if}
					</tr>
				{/each}
				{#if displayedRows.length === 0}
					<tr>
						<td
							class="text-surface-300 p-4 text-center"
							colspan={columns.length + (selectable ? 1 : 0) + (rowActions ? 1 : 0)}
						>
							{#if empty}
								{@render empty()}
							{:else}
								No results.
							{/if}
						</td>
					</tr>
				{/if}
			</tbody>
		</table>
	</div>

	<div class="flex items-center justify-between gap-2 text-sm">
		<span class="text-surface-300">
			{#if totalCount === 0}
				0 results
			{:else}
				{pageInfo.startIndex + 1}–{pageInfo.endIndex + 1} of {totalCount}
			{/if}
		</span>
		<div class="flex items-center gap-2">
			<button
				type="button"
				class="btn btn-sm btn-outline"
				disabled={!pageInfo.hasPrev}
				onclick={() => goToPage(page - 1)}
			>
				Prev
			</button>
			<span>Page {pageInfo.page} / {pageInfo.totalPages}</span>
			<button
				type="button"
				class="btn btn-sm btn-outline"
				disabled={!pageInfo.hasNext}
				onclick={() => goToPage(page + 1)}
			>
				Next
			</button>
		</div>
	</div>
</div>
