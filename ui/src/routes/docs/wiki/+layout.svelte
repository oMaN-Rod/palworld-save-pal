<script lang="ts">
	import { page } from '$app/state';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	const { children } = $props();

	const categories = [
		{ label: c.pal, href: '/docs/wiki/pals', id: 'pals' },
		{ label: c.item, href: '/docs/wiki/items', id: 'items' },
		{ label: m.buildings(), href: '/docs/wiki/buildings', id: 'buildings' },
		{ label: m.active_skill({ count: 2 }), href: '/docs/wiki/active-skills', id: 'active-skills' },
		{
			label: m.passive_skill({ count: 2 }),
			href: '/docs/wiki/passive-skills',
			id: 'passive-skills'
		},
		{
			label: m.technology({ count: 2 }),
			href: '/docs/wiki/technologies',
			id: 'technologies'
		},
		{ label: m.elements(), href: '/docs/wiki/elements', id: 'elements' },
		{ label: m.work_suitability(), href: '/docs/wiki/work-suitability', id: 'work-suitability' }
	];

	const activeCategory = $derived.by(() => {
		const path = page.url.pathname;
		const match = categories.find((c) => c.href !== '/docs/wiki' && path.startsWith(c.href));
		return match?.id || '';
	});
</script>

<div class="flex h-full">
	<aside class="flex w-48 shrink-0 flex-col gap-1 border-r border-surface-700 p-3">
		{#each categories as cat}
			<a
				href={cat.href}
				class="rounded-md px-3 py-1.5 text-sm transition-colors {activeCategory === cat.id
					? 'bg-surface-700 text-surface-50 font-medium'
					: 'text-surface-400 hover:bg-surface-800 hover:text-surface-200'}"
			>
				{cat.label}
			</a>
		{/each}
	</aside>
	<div class="flex-1 overflow-y-auto p-4">
		{@render children()}
	</div>
</div>
