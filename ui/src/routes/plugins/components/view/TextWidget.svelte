<script lang="ts">
	import { resolvePath, toText, type ViewWidget } from '$lib/plugins/pluginView';

	let { widget, results }: { widget: ViewWidget; results: Record<string, unknown> } = $props();

	const value = $derived(
		widget.from === null
			? (widget.text ?? '')
			: toText(resolvePath(results[widget.from], widget.path))
	);
</script>

<div>
	{#if widget.label}
		<div class="text-surface-400 text-xs">{widget.label}</div>
	{/if}
	<p class="text-sm whitespace-pre-wrap">{value}</p>
</div>
