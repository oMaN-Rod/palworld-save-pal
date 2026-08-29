<script lang="ts">
	import { Button } from '$components/ui';

	let {
		summary = null,
		onApply,
		onCancel
	}: {
		summary?: string | null;
		onApply: () => void;
		onCancel: () => void;
	} = $props();
</script>

<div
	class="border-warning-500/50 bg-surface-900 flex items-center justify-between gap-4 rounded-sm border p-3"
>
	<div class="min-w-0">
		<span class="text-warning-500 text-xs">Preview only -- nothing has been changed yet</span>
		{#if summary}
			<p class="truncate text-sm">{summary}</p>
		{/if}
	</div>
	<div class="flex shrink-0 gap-2">
		<Button variant="ghost" size="sm" onclick={onCancel}>Cancel</Button>
		<Button variant="secondary" size="sm" onclick={onApply}>Apply</Button>
	</div>
</div>
