<script lang="ts">
	import { Button } from '$components/ui';
	import { cn } from '$theme';
	import type { PluginRunResult } from '$types';

	let {
		result,
		pendingApply = false,
		onApply,
		onCancel
	}: {
		result: PluginRunResult;
		pendingApply?: boolean;
		onApply?: () => void;
		onCancel?: () => void;
	} = $props();

	const statusClass = $derived(
		result.status === 'ok'
			? 'text-success-500'
			: result.status === 'timeout' || result.status === 'cancelled' || result.status === 'memory_exceeded'
				? 'text-warning-500'
				: 'text-error-500'
	);

	const counts = $derived(Object.entries(result.counts ?? {}));

	function logLevelClass(level: string) {
		return cn(
			level === 'error' ? 'text-error-500' : level === 'warn' ? 'text-warning-500' : 'text-surface-300'
		);
	}
</script>

<div class="border-surface-700 flex flex-col gap-2 rounded-sm border p-3">
	<div class="flex items-center justify-between">
		<span class={cn('font-bold uppercase tracking-wide', statusClass)}>{result.status}</span>
		{#if pendingApply}
			<span class="text-warning-500 text-xs">Preview only -- nothing has been changed yet</span>
		{/if}
	</div>

	{#if result.message}
		<p class="text-sm">{result.message}</p>
	{/if}

	{#if result.summary}
		<p class="text-sm">{result.summary}</p>
	{/if}

	{#if counts.length > 0}
		<table class="text-sm">
			<tbody>
				{#each counts as [key, value] (key)}
					<tr>
						<td class="text-surface-400 pr-4">{key}</td>
						<td>{value}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}

	{#if result.log.length > 0}
		<div class="bg-surface-900 flex flex-col gap-1 h-full overflow-y-auto rounded-sm p-2 text-xs">
			{#each result.log as line, index (index)}
				<span class={logLevelClass(line.level)}>[{line.level}] {line.message}</span>
			{/each}
		</div>
	{/if}

	{#if pendingApply}
		<div class="flex justify-end gap-2">
			<Button variant="ghost" size="sm" onclick={onCancel}>Cancel</Button>
			<Button variant="secondary" size="sm" onclick={onApply}>Apply</Button>
		</div>
	{/if}
</div>
