<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { wikiPrefs, type WikiViewMode } from '$lib/utils/wikiPrefs.svelte';
	import { cn } from '$theme';

	const MODES: { id: WikiViewMode; icon: string; label: string }[] = [
		{ id: 'list', icon: 'tabler:layout-list', label: 'List view' },
		{ id: 'grid', icon: 'tabler:layout-grid', label: 'Grid view' }
	];
</script>

<div class="flex items-center gap-1">
	{#each MODES as mode (mode.id)}
		{@const active = wikiPrefs.viewMode === mode.id}
		<button
			type="button"
			class={cn(
				'btn btn-sm',
				active ? 'bg-secondary-500/25 text-surface-50' : 'text-surface-400'
			)}
			onclick={() => (wikiPrefs.viewMode = mode.id)}
			title={mode.label}
			aria-label={mode.label}
			aria-pressed={active}
		>
			<Icon icon={mode.icon} class="h-4 w-4" />
		</button>
	{/each}
</div>
