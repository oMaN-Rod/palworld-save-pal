<script lang="ts">
	import { WikiGrid, WikiSearch } from '$components/docs';
	import { descriptorFor, isDisabledRecord } from '$lib/utils/wikiDescriptors';
	import { categoryLabel, entityLink, type WikiCategory } from '$lib/utils/wikiCategories';

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
	<WikiGrid items={filteredEntries}>
		{#snippet toolbar()}
			<div class="flex items-center gap-3">
				<div class="min-w-48 flex-1">
					<WikiSearch bind:value={search} />
				</div>
				<span class="text-surface-400 text-xs">{filteredEntries.length}</span>
			</div>
		{/snippet}

		{#snippet children([key, record])}
			{@const name = descriptor.displayName(key, record)}
			{@const link = entityLink(data.category, key)}
			{@const icon = descriptor.icon?.(key, record) ?? null}
			{@const meta = descriptor.cardMeta?.(key, record) ?? null}
			<a
				href={link.href}
				class="border-surface-800 hover:border-primary-500/50 hover:bg-surface-700 flex flex-col items-center justify-center rounded-lg border p-3 text-center transition-colors"
			>
				{#if icon}
					<img
						src={icon.src}
						alt=""
						class="h-12 w-12 shrink-0 object-contain"
						style={icon.filter ? `filter: ${icon.filter};` : undefined}
					/>
				{/if}
				<span class="mt-2 line-clamp-2 text-sm font-medium">{name}</span>
				{#if meta}
					<span class="text-surface-400 mt-0.5 text-xs">{meta}</span>
				{/if}
			</a>
		{/snippet}
	</WikiGrid>
</div>
