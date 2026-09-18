<script lang="ts" module>
	export const menuItemClass =
		'hover:bg-surface-700 flex items-center gap-2 rounded-xs px-3 py-2 text-left text-sm disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:bg-transparent';
</script>

<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { Snippet } from 'svelte';
	import { Button, Popover } from '$components/ui';

	let {
		label,
		icon = 'tabler:dots',
		position = 'bottom-end',
		items
	}: {
		label: string;
		icon?: string;
		position?: 'bottom-end' | 'bottom-start';
		items: Snippet<[{ close: () => void }]>;
	} = $props();

	let wrapper: HTMLElement;

	/** Deferred because the popover portals the menu into the document after it is attached. */
	function focusFirstItem(node: HTMLElement) {
		queueMicrotask(() =>
			node.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)')?.focus()
		);
	}

	function closeAndRefocus(close: () => void) {
		close();
		wrapper.querySelector<HTMLElement>('[aria-haspopup="menu"]')?.focus();
	}
</script>

<div bind:this={wrapper} class="flex">
	<Popover {position} popoverClass="p-1 min-w-44">
		<Button variant="ghost" size="sm" aria-haspopup="menu" aria-label={label}>
			<Icon {icon} size={16} />
		</Button>
		{#snippet content({ close })}
			<div
				role="menu"
				tabindex="-1"
				class="flex flex-col"
				{@attach focusFirstItem}
				onkeydown={(event) => {
					if (event.key === 'Escape') closeAndRefocus(close);
				}}
			>
				{@render items({ close: () => closeAndRefocus(close) })}
			</div>
		{/snippet}
	</Popover>
</div>
