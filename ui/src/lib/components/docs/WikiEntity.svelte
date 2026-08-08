<script lang="ts">
	import type { Snippet } from 'svelte';
	import * as m from '$i18n/messages';
	import { categoryHref, type WikiCategory } from '$lib/utils/wikiCategories';

	let {
		category,
		title,
		subtitle,
		breadcrumbLabel,
		icons,
		media,
		infobox,
		children,
		related
	}: {
		category: WikiCategory;
		title: string;
		subtitle?: string;
		breadcrumbLabel: string;
		icons?: Snippet;
		media?: Snippet;
		infobox?: Snippet;
		children?: Snippet;
		related?: Snippet;
	} = $props();
</script>

<div class="w-full">
	<nav class="text-surface-400 mb-4 flex items-center gap-2 text-sm">
		<a href="/wiki" class="hover:text-surface-200">{m.docs_wiki()}</a>
		<span>/</span>
		<a href={categoryHref(category)} class="hover:text-surface-200">{breadcrumbLabel}</a>
		<span>/</span>
		<span class="text-surface-200">{title}</span>
	</nav>

	<div class="mb-5 flex items-center gap-3">
		<div>
			<h1 class="text-2xl font-bold">{title}</h1>
			{#if subtitle}
				<p class="text-surface-400 text-sm">{subtitle}</p>
			{/if}
		</div>
		{#if icons}
			<div class="flex items-center gap-2">{@render icons()}</div>
		{/if}
	</div>

	<div class="grid grid-cols-1 gap-6 lg:grid-cols-[minmax(0,360px)_minmax(0,1fr)]">
		{#if media}
			<div class="flex items-center justify-center">{@render media()}</div>
		{/if}
		<div class={media ? '' : 'lg:col-span-2'}>
			{#if infobox}
				<div class="mb-5">{@render infobox()}</div>
			{/if}
			{#if children}
				{@render children()}
			{/if}
		</div>
	</div>

	{#if related}
		<div class="border-surface-800 mt-8 border-t pt-5">
			{@render related()}
		</div>
	{/if}
</div>
