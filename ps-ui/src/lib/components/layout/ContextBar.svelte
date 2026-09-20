<script lang="ts" module>
	export type ContextBarItem = {
		id: string;
		label: string;
		href: string;
		available?: boolean;
	};
</script>

<script lang="ts">
	import * as m from '$i18n/messages';

	let { items, activeId, id }: { items: ContextBarItem[]; activeId: string; id?: string } =
		$props();

	const shown = $derived(items.filter((item) => item.available ?? true));
</script>

<nav
	{id}
	class="border-surface-700/40 bg-surface-900/60 scrollbar-none flex shrink-0 items-center gap-1.5 overflow-x-auto border-b px-3 py-2"
	aria-label={m.sections()}
>
	{#each shown as item (item.id)}
		<a
			href={item.href}
			aria-current={item.id === activeId ? 'page' : undefined}
			class="flex shrink-0 items-center rounded-full px-3.5 py-2 text-sm whitespace-nowrap transition-colors"
			class:context-bar-active={item.id === activeId}
			class:text-surface-300={item.id !== activeId}
		>
			{item.label}
		</a>
	{/each}
</nav>

<style>
	.scrollbar-none {
		scrollbar-width: none;
	}
	.scrollbar-none::-webkit-scrollbar {
		display: none;
	}
</style>
