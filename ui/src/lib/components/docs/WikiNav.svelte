<script lang="ts">
	import {
		categoryHref,
		categoryLabel,
		WIKI_CATEGORIES,
		type WikiCategory
	} from '$lib/utils/wikiCategories';

	let { active }: { active?: WikiCategory } = $props();
</script>

<nav class="border-surface-700 flex gap-1 overflow-x-auto border-b pb-2">
	{#each WIKI_CATEGORIES as category (category.id)}
		<a
			href={categoryHref(category.id)}
			aria-current={active === category.id ? 'page' : undefined}
			class="shrink-0 rounded-md px-3 py-1.5 text-sm whitespace-nowrap transition-colors {active ===
			category.id
				? 'bg-surface-700 text-surface-50 font-medium'
				: 'text-surface-400 hover:bg-surface-800 hover:text-surface-200'}"
		>
			{categoryLabel(category.id)}
		</a>
	{/each}
</nav>
