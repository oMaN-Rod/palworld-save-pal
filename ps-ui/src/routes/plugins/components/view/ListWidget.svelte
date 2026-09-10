<script lang="ts">
	import { resolvePath, toList, type ViewWidget } from '$lib/plugins/pluginView';

	let { widget, results }: { widget: ViewWidget; results: Record<string, unknown> } = $props();

	const data = $derived(
		toList(widget.from === null ? undefined : resolvePath(results[widget.from], widget.path))
	);
</script>

<div>
	{#if widget.label}
		<div class="text-surface-400 text-xs">{widget.label}</div>
	{/if}
	{#if data.items.length === 0}
		<p class="text-surface-400 text-sm">Nothing to show.</p>
	{:else}
		<ul class="list-inside list-disc text-sm">
			{#each data.items as item, index (index)}
				<li>{item}</li>
			{/each}
		</ul>
		{#if data.total > data.items.length}
			<p class="text-surface-400 text-xs">
				Showing {data.items.length} of {data.total}
			</p>
		{/if}
	{/if}
</div>
