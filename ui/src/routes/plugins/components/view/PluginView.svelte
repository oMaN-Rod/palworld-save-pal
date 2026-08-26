<script lang="ts">
	import type { Snippet } from 'svelte';
	import { cn } from '$theme';
	import type { PluginCommand } from '$types';
	import { viewGroups, type ViewGroup, type ViewWidget } from '$lib/plugins/pluginView';
	import type { PluginViewState } from '$lib/plugins/viewState.svelte';
	import ViewSection from './ViewSection.svelte';

	let {
		state: viewState,
		commands,
		disabled = false,
		onRun,
		children
	}: {
		state: PluginViewState;
		commands: PluginCommand[];
		disabled?: boolean;
		onRun: (widget: ViewWidget) => void;
		children?: Snippet;
	} = $props();

	const groups = $derived(viewGroups(viewState.sections));

	let selectedTitle = $state<string | null>(null);

	/// Falling back rather than storing the resolved title keeps the selection
	/// honest when the sections change under an open pane: a title the groups
	/// no longer have would otherwise leave the detail pane empty. Undefined
	/// only when a view declares no sections at all.
	const selected: ViewGroup | undefined = $derived(
		groups.find((g) => g.title === selectedTitle) ?? groups[0]
	);
</script>

{#snippet detail()}
	<div class="flex flex-col gap-4">
		{#each selected?.sections ?? [] as section, index (index)}
			<ViewSection {section} state={viewState} {commands} {disabled} {onRun} />
		{/each}
		{@render children?.()}
	</div>
{/snippet}

{#if groups.length > 1}
	<div class="grid grid-cols-[25%_1fr] gap-2">
		<div class="flex max-h-160 flex-col gap-2 overflow-y-auto 2xl:max-h-220">
			{#each groups as group (group.title)}
				<button
					type="button"
					aria-pressed={group === selected}
					class={cn(
						'rounded-sm border p-2 text-left font-medium transition-colors',
						group === selected
							? 'border-primary-500 bg-surface-800'
							: 'border-surface-700 hover:bg-secondary-500/25'
					)}
					onclick={() => (selectedTitle = group.title)}
				>
					{group.label}
				</button>
			{/each}
		</div>
		{@render detail()}
	</div>
{:else}
	{@render detail()}
{/if}
