<script lang="ts">
	import { cn } from '$theme';
	import { Checkbox } from '$components/ui';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import type { PluginSummary } from '$types';

	let {
		plugins,
		selectedId,
		onToggleEnabled
	}: {
		plugins: PluginSummary[];
		selectedId: string | undefined;
		onToggleEnabled: (id: string, enabled: boolean) => void;
	} = $props();
</script>

<div class="flex flex-col gap-1">
	{#each plugins as plugin (plugin.id)}
		<div
			class={cn(
				'flex items-center gap-2 rounded-md px-3 py-2 text-sm transition-colors',
				selectedId === plugin.id
					? 'bg-surface-700 text-surface-50 font-medium'
					: 'text-surface-400 hover:bg-surface-800 hover:text-surface-200'
			)}
		>
			<Checkbox
				checked={plugin.enabled}
				onchange={(event) => onToggleEnabled(plugin.id, (event.target as HTMLInputElement).checked)}
			/>
			<a
				href={`/plugins/${encodeURIComponent(plugin.id)}`}
				class="flex min-w-0 flex-1 items-center gap-2"
			>
				{#if plugin.error}
					<span
						class="text-error-500 shrink-0"
						role="img"
						aria-label={`Failed to load: ${plugin.error}`}
						title={plugin.error}
					>
						<TriangleAlert class="h-4 w-4" />
					</span>
				{/if}
				<span class="truncate">{plugin.name}</span>
				<span class="text-surface-500 shrink-0 text-xs">v{plugin.version}</span>
				<span
					class={cn(
						'shrink-0 rounded-full px-2 py-0.5 text-xs',
						plugin.bundled
							? 'bg-secondary-500/25 text-secondary-300'
							: 'bg-primary-500/25 text-primary-300'
					)}
				>
					{plugin.bundled ? 'Bundled' : 'User'}
				</span>
			</a>
		</div>
	{/each}
</div>
