<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { ActionGroup, type ActionDescriptor } from '$components/ui/actions';
	import { layout } from '$utils/layout.svelte';
	import { cn } from '$theme';
	import * as m from '$i18n/messages';

	import type { PalContainerViewMode } from '$states/palContainer.svelte';

	let {
		viewMode = $bindable(),
		selectionCount,
		actions,
		filterable = true,
		filterOpen = false,
		filterPanelId,
		onOpenFilter,
		title
	}: {
		viewMode: PalContainerViewMode;
		selectionCount: number;
		actions: ActionDescriptor[];
		/** False when the panel behind the button would have nothing in it. */
		filterable?: boolean;
		filterOpen?: boolean;
		filterPanelId?: string;
		onOpenFilter: () => void;
		title: string;
	} = $props();

	const viewModes: { mode: PalContainerViewMode; label: string; icon: string }[] = [
		{ mode: 'grid', label: m.grid_view(), icon: 'tabler:layout-grid' },
		{ mode: 'list', label: m.list_view(), icon: 'tabler:layout-list' }
	];

	const filterLabel = m.filter({ count: 1 });
</script>

<div class="flex flex-col gap-2">
	<div class="flex items-center justify-between gap-2">
		<h2 class="h4 truncate">{title}</h2>

		<div class="flex items-center gap-2">
			{#if filterable}
				<button
					type="button"
					class="btn preset-outlined-surface-200-800 flex items-center gap-1.5"
					aria-label={filterLabel}
					aria-expanded={filterOpen}
					aria-controls={filterOpen ? filterPanelId : undefined}
					onclick={onOpenFilter}
				>
					<Icon icon="tabler:filter" class="size-5" />
					{#if !layout.phone}<span>{filterLabel}</span>{/if}
				</button>
			{/if}

			<div
				class="btn-group preset-outlined-surface-200-800 rounded-sm"
				role="group"
				aria-label={title}
			>
				{#each viewModes as { mode, label, icon } (mode)}
					<button
						type="button"
						class={cn('btn flex items-center gap-1.5', viewMode === mode ? 'preset-filled' : '')}
						aria-label={label}
						aria-pressed={viewMode === mode}
						onclick={() => (viewMode = mode)}
					>
						<Icon {icon} class="size-5" />
						{#if !layout.phone}<span>{label}</span>{/if}
					</button>
				{/each}
			</div>
		</div>
	</div>

	<!-- Pages hide their own quick actions once something is selected, so this is the only way to reach them. -->
	{#if selectionCount > 0}
		<div class="bg-surface-800 flex items-center justify-between gap-2 rounded-sm px-3 py-2">
			<span class="text-sm font-bold">
				{m.pals_selected_count({ count: selectionCount, pals: m.pal({ count: selectionCount }) })}
			</span>
			<ActionGroup {actions} title={m.bulk_actions()} id="pal-container-actions" class="flex-row" />
		</div>
	{/if}
</div>
