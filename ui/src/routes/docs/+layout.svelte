<script lang="ts">
	import { page } from '$app/state';
	import * as m from '$i18n/messages';

	const { children } = $props();

	const tabs = [
		{ label: m.docs_wiki(), href: '/docs/wiki', id: 'wiki' },
		{ label: m.docs_guides(), href: '/docs/guides', id: 'guides' },
		{ label: m.docs_tours(), href: '/docs/tours', id: 'tours' },
	];

	const activeTab = $derived.by(() => {
		if (page.url.pathname.startsWith('/docs/guides')) return 'guides';
		if (page.url.pathname.startsWith('/docs/tours')) return 'tours';
		return 'wiki';
	});
</script>

<div class="flex h-full w-full flex-col overflow-hidden">
	<nav class="flex gap-1 border-b border-surface-700 px-4 pt-2">
		{#each tabs as tab}
			<a
				href={tab.href}
				class="rounded-t-md px-4 py-2 text-sm font-medium transition-colors {activeTab === tab.id
					? 'bg-surface-700 text-surface-50'
					: 'text-surface-400 hover:bg-surface-800 hover:text-surface-200'}"
			>
				{tab.label}
			</a>
		{/each}
	</nav>
	<div class="flex-1 overflow-y-auto">
		{@render children()}
	</div>
</div>
