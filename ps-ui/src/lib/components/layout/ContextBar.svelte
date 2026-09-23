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
	class="bg-surface-900/40 scrollbar-none flex shrink-0 overflow-x-auto"
	aria-label={m.sections()}
>
	{#each shown as item (item.id)}
		<a
			id={item.id}
			href={item.href}
			aria-current={item.id === activeId ? 'page' : undefined}
			class="flex h-11 min-w-fit flex-1 items-center justify-center px-3 text-sm whitespace-nowrap transition-colors"
			class:font-bold={item.id === activeId}
			class:text-primary-300={item.id === activeId}
			class:border-primary-400={item.id === activeId}
			class:border-b-2={item.id === activeId}
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
