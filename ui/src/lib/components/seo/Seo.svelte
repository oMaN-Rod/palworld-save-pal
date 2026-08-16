<script lang="ts">
	import { getLocale } from '$i18n/runtime';
	import {
		SITE_ORIGIN,
		hrefLanguageTags,
		isLocalizedPath,
		localizedPath,
		siteLocales
	} from '$lib/i18n/routingConfig.js';

	type SiteLocale = (typeof siteLocales)[number];

	let {
		pathname,
		title,
		description,
		ogTitle = title,
		ogDescription = description,
		ogImage = `${SITE_ORIGIN}/psp.png`,
		noindex = false,
		structuredData
	}: {
		pathname: string;
		title: string;
		description: string;
		ogTitle?: string;
		ogDescription?: string;
		ogImage?: string;
		noindex?: boolean;
		structuredData?: Record<string, unknown> | Record<string, unknown>[];
	} = $props();

	const locale = $derived(getLocale() as SiteLocale);
	const localized = $derived(isLocalizedPath(pathname));
	const canonical = $derived(
		`${SITE_ORIGIN}${localized ? localizedPath(pathname, locale) : pathname}`
	);
	const robots = $derived(noindex ? 'noindex,nofollow' : 'index,follow,max-image-preview:large');

	// Pages outside the hub set exist only in English; advertising alternates for
	// them would point crawlers at URLs that do not exist.
	const alternates = $derived(
		localized
			? siteLocales.map((alt) => ({
					locale: alt,
					hreflang: hrefLanguageTags[alt],
					href: `${SITE_ORIGIN}${localizedPath(pathname, alt)}`
				}))
			: []
	);

	const jsonLd = $derived.by(() => {
		if (!structuredData) return '';
		const blocks = Array.isArray(structuredData) ? structuredData : [structuredData];
		const payload = blocks.map((block) => ({
			'@context': 'https://schema.org',
			...block,
			url: canonical
		}));
		const body = JSON.stringify(payload.length === 1 ? payload[0] : payload).replace(
			/</g,
			'\\u003c'
		);
		return `<script type="application/ld+json">${body}<\/script>`;
	});
</script>

<svelte:head>
	<title>{title}</title>
	<meta name="description" content={description} />
	<meta name="robots" content={robots} />
	<link rel="canonical" href={canonical} />

	{#each alternates as alternate (alternate.locale)}
		<link rel="alternate" hreflang={alternate.hreflang} href={alternate.href} />
	{/each}
	{#if localized}
		<link
			rel="alternate"
			hreflang="x-default"
			href={`${SITE_ORIGIN}${localizedPath(pathname, 'en')}`}
		/>
	{/if}

	<meta property="og:site_name" content="Palworld Save Pal" />
	<meta property="og:type" content="website" />
	<meta property="og:title" content={ogTitle} />
	<meta property="og:description" content={ogDescription} />
	<meta property="og:url" content={canonical} />
	<meta property="og:image" content={ogImage} />
	<meta property="og:locale" content={hrefLanguageTags[locale]} />

	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:title" content={ogTitle} />
	<meta name="twitter:description" content={ogDescription} />
	<meta name="twitter:image" content={ogImage} />

	{#if jsonLd}
		{@html jsonLd}
	{/if}
</svelte:head>
