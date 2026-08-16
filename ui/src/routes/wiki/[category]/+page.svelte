<script lang="ts">
	import { WikiGrid, WikiSearch, WikiCard, WikiViewToggle, ElementExplorer, PassiveSkillExplorer, WorkSuitabilityExplorer } from '$components/docs';
	import { descriptorFor, isDisabledRecord } from '$lib/utils/wikiDescriptors';
	import { categoryLabel, entityLink, type WikiCategory } from '$lib/utils/wikiCategories';
	import { wikiPrefs } from '$lib/utils/wikiPrefs.svelte';

	let { data }: { data: { category: WikiCategory } } = $props();

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
				descriptor.displayName(key, record).toLowerCase().includes(q) || key.toLowerCase().includes(q)
		);
	});
</script>

<svelte:head>
	<title>{categoryLabel(data.category)} | Palworld Save Pal Wiki</title>
</svelte:head>

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
