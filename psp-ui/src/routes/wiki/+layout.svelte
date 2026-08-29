<script lang="ts">
	import { page } from '$app/state';
	import { WikiNav } from '$components/docs';
	import { WIKI_CATEGORIES, type WikiCategory } from '$lib/utils/wikiCategories';

	let { children } = $props();

	// Second path segment names the category on both index and entity routes;
	// the hub has none, which leaves every tab unhighlighted.
	const active = $derived.by(() => {
		const segment = page.url.pathname.split('/')[2];
		return WIKI_CATEGORIES.some((category) => category.id === segment)
			? (segment as WikiCategory)
			: undefined;
	});
</script>

<div class="p-5">
	<WikiNav {active} />
	<div class="mt-4">
		{@render children()}
	</div>
</div>
