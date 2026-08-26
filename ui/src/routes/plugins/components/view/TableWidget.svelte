<script lang="ts">
	import { resolvePath, selectAllState, toRows, type ViewWidget } from '$lib/plugins/pluginView';
	import type { PluginViewState } from '$lib/plugins/viewState.svelte';

	let { widget, state }: { widget: ViewWidget; state: PluginViewState } = $props();

	const data = $derived(
		toRows(
			widget.from === null ? undefined : resolvePath(state.results[widget.from], widget.path),
			widget.columns
		)
	);
	const selected = $derived(widget.id ? (state.selections[widget.id] ?? []) : []);
	const allState = $derived(selectAllState(data.ids, selected));
</script>

<div class="flex flex-col gap-1">
	{#if widget.label}
		<div class="text-surface-400 text-xs">{widget.label}</div>
	{/if}
	{#if data.rows.length === 0}
		<p class="text-surface-400 text-sm">Nothing to show.</p>
	{:else}
		<div class="border-surface-700 max-h-96 overflow-auto rounded-sm border">
			<table class="w-full text-sm">
				<thead class="bg-surface-800 sticky top-0">
					<tr>
						{#if widget.selectable}
							<th class="w-8 p-1">
								<input
									type="checkbox"
									aria-label={allState === 'all' ? 'Deselect all rows' : 'Select all rows'}
									checked={allState === 'all'}
									indeterminate={allState === 'some'}
									onchange={() =>
										widget.id &&
										state.setSelection(widget.id, allState === 'all' ? [] : [...data.ids])}
								/>
							</th>
						{/if}
						{#each data.columns as column, columnIndex (columnIndex)}
							<th class="p-1 text-left font-medium">{column}</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each data.rows as row, index (index)}
						<tr class="border-surface-800 border-t">
							{#if widget.selectable}
								<td class="p-1">
									<input
										type="checkbox"
										aria-label={`Select row ${index + 1}`}
										checked={selected.includes(data.ids[index])}
										onchange={() => widget.id && state.toggleRow(widget.id, data.ids[index])}
									/>
								</td>
							{/if}
							{#each data.columns as column, columnIndex (columnIndex)}
								<td class="p-1">{row[column]}</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		{#if data.total > data.rows.length}
			<p class="text-surface-400 text-xs">
				Showing {data.rows.length} of {data.total}
			</p>
		{/if}
		{#if widget.selectable}
			<p class="text-surface-400 text-xs">{selected.length} selected</p>
		{/if}
	{/if}
</div>
