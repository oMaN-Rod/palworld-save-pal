<script lang="ts">
	import { page } from '$app/state';
	import { Lightbox, TableOfContents } from '$components/docs';

	const { children } = $props();

	let lightboxSrc = $state('');
	let lightboxAlt = $state('');

	function onProseClick(e: MouseEvent) {
		const img = (e.target as HTMLElement).closest('.prose-psp img') as HTMLImageElement | null;
		if (img) {
			lightboxSrc = img.src;
			lightboxAlt = img.alt || '';
		}
	}

	const guides = [
		{ label: 'All Guides', href: '/docs/guides', slug: '' },
		{ label: 'Server Setup', href: '/docs/guides/server-setup', slug: 'server-setup' },
		{ label: 'Save Management', href: '/docs/guides/save-management', slug: 'save-management' }
	];

	const activeGuide = $derived.by(() => {
		const path = page.url.pathname;
		const match = guides.find((g) => g.href !== '/docs/guides' && path === g.href);
		return match?.slug || '';
	});

	const showToc = $derived(activeGuide !== '');
</script>

<div class="flex h-full overflow-hidden">
	<aside class="flex w-48 shrink-0 flex-col gap-1 border-r border-surface-700 p-3 overflow-y-auto">
		{#each guides as guide}
			<a
				href={guide.href}
				class="rounded-md px-3 py-1.5 text-sm transition-colors {activeGuide === guide.slug
					? 'bg-surface-700 text-surface-50 font-medium'
					: 'text-surface-400 hover:bg-surface-800 hover:text-surface-200'}"
			>
				{guide.label}
			</a>
		{/each}
	</aside>
	
	<main class="prose-psp min-w-0 flex-1 overflow-y-auto p-6 bg-surface-900/25" onclick={onProseClick}>
		{@render children()}
	</main>
	{#if showToc}
		<aside class="hidden w-56 shrink-0 overflow-y-auto border-l border-surface-700 p-4 xl:block">
			<TableOfContents />
		</aside>
	{/if}
	<Lightbox bind:src={lightboxSrc} bind:alt={lightboxAlt} />
</div>
