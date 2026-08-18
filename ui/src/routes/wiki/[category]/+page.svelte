<script lang="ts">
	import {
		WikiGrid,
		WikiSearch,
		WikiCard,
		WikiViewToggle,
		ElementExplorer,
		PassiveSkillExplorer,
		WorkSuitabilityExplorer
	} from '$components/docs';
	import { Seo, breadcrumbSchema, itemListSchema } from '$lib/components/seo';
	import { descriptorFor, isDisabledRecord } from '$lib/utils/wikiDescriptors';
	import {
		categoryLabel,
		categoryLabelPlural,
		entityLink,
		type WikiCategory
	} from '$lib/utils/wikiCategories';
	import { wikiPrefs } from '$lib/utils/wikiPrefs.svelte';

	let { data }: { data: { category: WikiCategory; names: string[]; slugs: string[] } } = $props();

	let search = $state('');

	const descriptor = $derived(descriptorFor(data.category));
	const allEntries = $derived(
		(Object.entries(descriptor.runtime()) as [string, Record<string, unknown>][]).filter(
			([, record]) => !isDisabledRecord(record)
		)
	);

	const filteredEntries = $derived.by(() => {
		if (!search) return allEntries;
		const q = search.toLowerCase();
		return allEntries.filter(
			([key, record]) =>
				descriptor.displayName(key, record).toLowerCase().includes(q) ||
				key.toLowerCase().includes(q)
		);
	});
</script>

<Seo
	pathname={`/wiki/${data.category}`}
	title={`Palworld ${categoryLabelPlural(data.category)} - Complete List`}
	description={`Every ${categoryLabel(data.category).toLowerCase()} entry in Palworld, with stats and cross-references from the game data.`}
	structuredData={[
		breadcrumbSchema([
			{ name: 'Wiki', path: '/wiki' },
			{ name: categoryLabel(data.category), path: `/wiki/${data.category}` }
		]),
		itemListSchema(
			categoryLabel(data.category),
			data.names.map((name, index) => ({
				name,
				path: `/wiki/${data.category}/${data.slugs[index]}`
			}))
		)
	]}
/>

<!-- Prerender-time listing: the runtime store is empty, so the interactive grid
     below renders nothing. This gives crawlers the full set of entity links. -->
{#if allEntries.length === 0}
	<div class="p-5">
		<h1 class="mb-4 text-2xl font-bold">{categoryLabelPlural(data.category)}</h1>
		<ul class="grid grid-cols-2 gap-x-6 gap-y-1 sm:grid-cols-3 lg:grid-cols-4">
			{#each data.names as name, index (data.slugs[index])}
				<li>
					<a
						class="text-surface-300 hover:underline"
						href={`/wiki/${data.category}/${data.slugs[index]}`}
					>
						{name}
					</a>
				</li>
			{/each}
		</ul>
	</div>
{/if}

<div>
	{#if data.category === 'elements'}
		<ElementExplorer />
	{:else if data.category === 'work-suitability'}
		<WorkSuitabilityExplorer />
	{:else if data.category === 'passive-skills'}
		<PassiveSkillExplorer />
	{:else}
		<WikiGrid items={filteredEntries}>
			{#snippet toolbar()}
				<div class="flex items-center gap-3">
					<div class="min-w-48 flex-1">
						<WikiSearch bind:value={search} />
					</div>
					<WikiViewToggle />
					<span class="text-surface-400 text-xs">{filteredEntries.length}</span>
				</div>
			{/snippet}

			{#snippet children([key, record])}
				{@const name = descriptor.displayName(key, record)}
				{@const link = entityLink(data.category, key)}
				{@const icon = descriptor.icon?.(key, record) ?? null}
				{@const meta = descriptor.cardMeta?.(key, record) ?? null}
				<WikiCard href={link.href} {name} {icon} {meta} variant={wikiPrefs.viewMode} />
			{/snippet}
		</WikiGrid>
	{/if}
</div>
