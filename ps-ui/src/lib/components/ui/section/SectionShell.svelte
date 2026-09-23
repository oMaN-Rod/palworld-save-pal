<script lang="ts">
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import type { ValueChangeDetails } from '@zag-js/accordion';
	import SectionTabs, { panelId, tabId } from '$components/ui/tabs/SectionTabs.svelte';
	import type { SectionDef, SectionPresentation } from './sectionShell';

	let {
		sections,
		presentation,
		active = $bindable(sections[0]?.id),
		label,
		idPrefix
	}: {
		sections: SectionDef[];
		presentation: SectionPresentation;
		active?: string;
		label: string;
		idPrefix: string;
	} = $props();

	let accordionValue: string[] = $state([]);

	const columnGroups = $derived.by(() => {
		const groups = new Map<SectionDef['group'], SectionDef[]>();
		for (const section of sections) {
			const group = groups.get(section.group);
			if (group) {
				group.push(section);
			} else {
				groups.set(section.group, [section]);
			}
		}
		return [...groups.values()];
	});
</script>

{#if presentation === 'columns'}
	<div
		class="grid gap-2"
		style="grid-template-columns: repeat({columnGroups.length}, minmax(0, 1fr));"
	>
		{#each columnGroups as group, index (index)}
			<div class="flex flex-col gap-2">
				{#each group as section (section.id)}
					<div data-testid={section.id}>
						{#if section.header}
							{@render section.header()}
						{/if}
						{@render section.body()}
					</div>
				{/each}
			</div>
		{/each}
	</div>
{:else if presentation === 'accordion'}
	<Accordion
		value={accordionValue}
		onValueChange={(e: ValueChangeDetails) => (accordionValue = e.value)}
		collapsible
	>
		{#each sections as section (section.id)}
			<Accordion.Item value={section.id} controlHover="hover:bg-secondary-500/25">
				{#snippet control()}
					<div data-testid={section.id}>
						{#if section.header}
							{@render section.header()}
						{:else}
							{section.title}
						{/if}
					</div>
				{/snippet}
				{#snippet panel()}
					{@render section.body()}
				{/snippet}
			</Accordion.Item>
		{/each}
	</Accordion>
{:else}
	<SectionTabs
		tabs={sections.map((section) => ({ id: section.id, label: section.title }))}
		bind:active
		{label}
		{idPrefix}
	/>
	{#each sections as section (section.id)}
		{#if section.id === active}
			<div
				id={panelId(idPrefix, section.id)}
				role="tabpanel"
				aria-labelledby={tabId(idPrefix, section.id)}
				tabindex="0"
				data-testid={section.id}
			>
				{@render section.body()}
			</div>
		{/if}
	{/each}
{/if}
