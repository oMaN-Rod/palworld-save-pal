<script lang="ts" generics="TPal, TId extends string | number">
	import type { Snippet } from 'svelte';

	import { longPress } from '$lib/actions/longPress';
	import { cn } from '$theme';
	import * as m from '$i18n/messages';

	/**
	 * The button is a stretched overlay, not a wrapper, because a `<button>`
	 * flattens its children into its accessible name and would hide `columns`
	 * from assistive tech. `portrait` and `columns` must be non-interactive.
	 */
	let {
		pals,
		idOf,
		nicknameOf,
		selectedIds,
		onSelect,
		onLongPress,
		portrait,
		columns
	}: {
		pals: TPal[];
		idOf: (pal: TPal) => TId;
		nicknameOf: (pal: TPal) => string;
		selectedIds: TId[];
		onSelect: (pal: TPal) => void;
		onLongPress: (pal: TPal) => void;
		portrait: Snippet<[TPal]>;
		columns: Snippet<[TPal]>;
	} = $props();

	const selectedSet = $derived(new Set(selectedIds));
</script>

<ul class="space-y-2 p-4">
	{#each pals as pal (idOf(pal))}
		{@const selected = selectedSet.has(idOf(pal))}
		<li
			class={cn(
				'card-hover hover:border-primary-500/40 hover:bg-surface-800/75 border-surface-700/60 bg-surface-800/50 relative flex items-center gap-4 rounded-sm border p-3 transition-colors',
				selected ? 'border-secondary-500/60 bg-secondary-500/10 ring-secondary-500 ring-2' : ''
			)}
		>
			<div class="pointer-events-none h-16 w-16 shrink-0">
				{@render portrait(pal)}
			</div>

			<div class="pointer-events-none min-w-0 flex-1">
				{@render columns(pal)}
			</div>

			<button
				type="button"
				class="focus-visible:outline-primary-300 absolute inset-0 w-full cursor-pointer rounded-sm focus-visible:outline-2 focus-visible:outline-offset-2"
				onclick={() => onSelect(pal)}
				use:longPress={{ onLongPress: () => onLongPress(pal) }}
			>
				<span class="sr-only"
					>{selected ? `${nicknameOf(pal)}, ${m.selected()}` : nicknameOf(pal)}</span
				>
			</button>
		</li>
	{/each}
</ul>
