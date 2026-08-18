<script lang="ts">
	import { wikiPrefs, type WikiViewMode } from '$lib/utils/wikiPrefs.svelte';
	import { cn } from '$theme';
	import LayoutGrid from '@lucide/svelte/icons/layout-grid';
	import LayoutList from '@lucide/svelte/icons/layout-list';

	const MODES: { id: WikiViewMode; icon: typeof LayoutList; label: string }[] = [
		{ id: 'list', icon: LayoutList, label: 'List view' },
		{ id: 'grid', icon: LayoutGrid, label: 'Grid view' }
	];
</script>

<div class="flex items-center gap-1">
	{#each MODES as mode (mode.id)}
		{@const active = wikiPrefs.viewMode === mode.id}
		<button
			type="button"
			class={cn('btn btn-sm', active ? 'bg-secondary-500/25 text-surface-50' : 'text-surface-400')}
			onclick={() => (wikiPrefs.viewMode = mode.id)}
			title={mode.label}
			aria-label={mode.label}
			aria-pressed={active}
		>
			<mode.icon class="h-4 w-4" />
		</button>
	{/each}
</div>
