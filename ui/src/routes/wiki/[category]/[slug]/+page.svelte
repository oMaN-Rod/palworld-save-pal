<script lang="ts">
	import { WikiEntity } from '$components/docs';
	import { Seo, breadcrumbSchema } from '$lib/components/seo';
	import { Loading } from '$components/ui';
	import { DESCRIPTORS } from '$lib/utils/wikiDescriptors';
	import { categoryLabel, entityLink, type WikiCategory } from '$lib/utils/wikiCategories';
	import { buildSlugIndex, keyFromSlug, stripKeyPrefix } from '$lib/utils/wikiSlug';
	import * as m from '$i18n/messages';

	let {
		data
	}: {
		data: { category: string; slug: string; name: string; description: string | null };
	} = $props();

	const category = $derived(data.category as WikiCategory);
	const descriptor = $derived(DESCRIPTORS[category]);

	const runtimeRecords = $derived(descriptor?.runtime() ?? {});
	const runtimeKeys = $derived(Object.keys(runtimeRecords));
	const hasData = $derived(runtimeKeys.length > 0);

	const strippedToRawKey = $derived(new Map(runtimeKeys.map((key) => [stripKeyPrefix(key), key])));
	const slugIndex = $derived(buildSlugIndex([...strippedToRawKey.keys()]));
	const matchedStrippedKey = $derived(keyFromSlug(data.slug, slugIndex));
	const rawKey = $derived(
		matchedStrippedKey ? strippedToRawKey.get(matchedStrippedKey) : undefined
	);
	const record = $derived(rawKey ? (runtimeRecords[rawKey] as Record<string, unknown>) : undefined);

	const title = $derived(record && rawKey ? descriptor.displayName(rawKey, record) : data.slug);
	const description = $derived(
		record && descriptor.description ? descriptor.description(record) : null
	);
	const icon = $derived(
		record && rawKey && descriptor.icon ? descriptor.icon(rawKey, record) : null
	);
	const relatedItems = $derived(
		record && rawKey && descriptor.related ? descriptor.related(rawKey, record) : []
	);
	const extras = $derived(
		record && rawKey && descriptor.extras ? descriptor.extras(rawKey, record) : []
	);
</script>

<Seo
	pathname={`/wiki/${data.category}/${data.slug}`}
	title={`${data.name} - Palworld ${categoryLabel(category)}`}
	description={data.description ??
		`${data.name} details, stats and related entries from the Palworld game data.`}
	structuredData={breadcrumbSchema([
		{ name: 'Wiki', path: '/wiki' },
		{ name: categoryLabel(category), path: `/wiki/${data.category}` },
		{ name: data.name, path: `/wiki/${data.category}/${data.slug}` }
	])}
/>

<!-- Rendered during prerender, when the runtime store is still empty. This is
     the content crawlers receive; hydration replaces it with the full view. -->
{#if !hasData}
	<div class="p-5">
		<h1 class="text-2xl font-bold">{data.name}</h1>
		{#if data.description}
			<p class="text-surface-300 mt-2 max-w-2xl">{data.description}</p>
		{/if}
	</div>
{/if}

{#snippet media()}
	<div class="flex flex-col items-center gap-4">
		{#if icon}
			<div
				class="bg-surface-900 flex h-40 w-40 items-center justify-center rounded-full"
				style={icon.color ? `background-color: ${icon.color}33` : ''}
			>
				<img
					src={icon.src}
					alt={title}
					class="max-h-28 max-w-28"
					style={icon.filter ? `filter: ${icon.filter};` : undefined}
				/>
			</div>
		{/if}
		{#if extras.length > 0}
			<div class="flex flex-wrap justify-center gap-3">
				{#each extras as extra (extra.label)}
					<div class="flex flex-col items-center gap-1">
						<img src={extra.src} alt={extra.label} class="h-16 w-16 object-contain" />
						<span class="text-surface-400 text-xs">{extra.label}</span>
					</div>
				{/each}
			</div>
		{/if}
	</div>
{/snippet}

{#snippet relatedSnippet()}
	{#if relatedItems.length > 0}
		<h3 class="text-surface-400 mb-2 text-sm font-semibold">Related</h3>
		<div class="flex flex-wrap gap-2">
			{#each relatedItems as item (`${item.category}-${item.key}`)}
				{@const link = entityLink(item.category, item.key)}
				{#snippet chipBody()}
					{#if item.icon}
						<img
							src={item.icon.src}
							alt=""
							class="h-8 w-8 shrink-0 object-contain"
							style={item.icon.filter ? `filter: ${item.icon.filter};` : undefined}
						/>
					{/if}
					<span class="flex flex-col leading-tight">
						<span>{item.label}</span>
						{#if item.sublabel}
							<span class="text-surface-400 font-mono text-xs">{item.sublabel}</span>
						{/if}
					</span>
				{/snippet}
				{#if item.missing}
					<span
						class="bg-surface-900 text-surface-400 flex items-center gap-2 rounded-md px-3 py-1.5 text-sm"
					>
						{@render chipBody()}
					</span>
				{:else}
					<a
						href={link.href}
						class="bg-surface-900 hover:bg-surface-800 flex items-center gap-2 rounded-md px-3 py-1.5 text-sm"
					>
						{@render chipBody()}
					</a>
				{/if}
			{/each}
		</div>
	{/if}
{/snippet}

{#if !descriptor}
	<div class="text-surface-400 flex items-center justify-center py-12">
		<p>{m.docs_no_results()}</p>
	</div>
{:else if record && rawKey}
	<WikiEntity
		{category}
		{title}
		subtitle={rawKey}
		breadcrumbLabel={categoryLabel(category)}
		media={icon ? media : undefined}
		related={relatedItems.length > 0 ? relatedSnippet : undefined}
	>
		{#snippet infobox()}
			{#if description}
				<p class="text-surface-300 mb-4">{description}</p>
			{/if}
			<div class="space-y-1">
				{#each descriptor.fields as fieldDef (fieldDef.label)}
					{@const value = fieldDef.value(record)}
					{#if value !== null}
						<div class="border-surface-800 flex justify-between gap-4 border-b py-1.5 text-sm">
							<span class="text-surface-400">{fieldDef.label}</span>
							<span class="text-surface-100 text-right">{value}</span>
						</div>
					{/if}
				{/each}
			</div>
		{/snippet}
	</WikiEntity>
{:else if hasData}
	<div class="text-surface-400 flex items-center justify-center py-12">
		<p>{m.docs_no_results()}</p>
	</div>
{:else}
	<Loading label={m.loading_entity({ entity: categoryLabel(category) })} />
{/if}
