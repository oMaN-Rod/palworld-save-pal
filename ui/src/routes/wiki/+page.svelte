<script lang="ts">
	import * as m from '$i18n/messages';
	import { Seo, breadcrumbSchema } from '$lib/components/seo';
	import {
		palsData,
		itemsData,
		buildingsData,
		activeSkillsData,
		passiveSkillsData,
		technologiesData,
		elementsData,
		workSuitabilityData
	} from '$lib/data';
	import { WikiCard, WikiSearch } from '$components/docs';
	import { searchWiki, type WikiSearchEntry } from '$lib/utils/wikiSearch';
	import { descriptorFor } from '$lib/utils/wikiDescriptors';
	import {
		categoryHref,
		categoryLabel,
		entityLink,
		WIKI_CATEGORIES,
		type WikiCategory
	} from '$lib/utils/wikiCategories';
	import Egg from '@lucide/svelte/icons/egg';
	import Package from '@lucide/svelte/icons/package';
	import Building from '@lucide/svelte/icons/building';
	import Swords from '@lucide/svelte/icons/swords';
	import Shield from '@lucide/svelte/icons/shield';
	import FlaskConical from '@lucide/svelte/icons/flask-conical';
	import Flame from '@lucide/svelte/icons/flame';
	import Hammer from '@lucide/svelte/icons/hammer';

	let query = $state('');

	const categoryMeta: Record<WikiCategory, { icon: typeof Egg; description: string }> = {
		pals: {
			icon: Egg,
			description: 'Stats, elements, skills, and work suitabilities for all Pals.'
		},
		items: {
			icon: Package,
			description: 'All items including weapons, armor, consumables, and materials.'
		},
		buildings: { icon: Building, description: 'Building recipes, materials, and stats.' },
		'active-skills': {
			icon: Swords,
			description: 'Combat skills with element types, power, and cooldowns.'
		},
		'passive-skills': { icon: Shield, description: 'Passive abilities and their stat effects.' },
		technologies: {
			icon: FlaskConical,
			description: 'Technology tree, unlock requirements, and costs.'
		},
		elements: { icon: Flame, description: 'Element types and their properties.' },
		'work-suitability': { icon: Hammer, description: 'Work types and which Pals excel at each.' }
	};

	const entriesByCategory = $derived.by(() => {
		const map: Record<WikiCategory, WikiSearchEntry[]> = {
			pals: Object.entries(palsData.pals).map(([key, pal]) => ({
				category: 'pals',
				key,
				name: pal.localized_name || key
			})),
			items: Object.entries(itemsData.items).map(([key, item]) => ({
				category: 'items',
				key,
				name: item.info?.localized_name || key
			})),
			buildings: Object.entries(buildingsData.buildings).map(([key, building]) => ({
				category: 'buildings',
				key,
				name: building.localized_name || key
			})),
			'active-skills': Object.entries(activeSkillsData.activeSkills).map(([key, skill]) => ({
				category: 'active-skills',
				key,
				name: skill.localized_name || key
			})),
			'passive-skills': Object.entries(passiveSkillsData.passiveSkills).map(([key, skill]) => ({
				category: 'passive-skills',
				key,
				name: skill.localized_name || key
			})),
			technologies: Object.entries(technologiesData.technologies).map(([key, tech]) => ({
				category: 'technologies',
				key,
				name: tech.localized_name || key
			})),
			elements: Object.entries(elementsData.elements).map(([key, element]) => ({
				category: 'elements',
				key,
				name: element.localized_name || key
			})),
			'work-suitability': Object.entries(workSuitabilityData.workSuitability).map(
				([key, suit]) => ({
					category: 'work-suitability',
					key,
					name: suit.localized_name || key
				})
			)
		};
		return map;
	});

	const allEntries = $derived(WIKI_CATEGORIES.flatMap((cat) => entriesByCategory[cat.id]));

	const results = $derived(searchWiki(query, allEntries));

	const resultsByCategory = $derived.by(() => {
		const grouped = new Map<WikiCategory, WikiSearchEntry[]>();
		for (const result of results) {
			const list = grouped.get(result.category) ?? [];
			list.push(result);
			grouped.set(result.category, list);
		}
		return grouped;
	});
</script>

<Seo
	pathname="/wiki"
	title={m.wiki_meta_title()}
	description={m.wiki_meta_description()}
	structuredData={breadcrumbSchema([{ name: 'Wiki', path: '/wiki' }])}
/>

<div>
	<h1 class="mb-2 text-2xl font-bold">{m.docs_wiki()}</h1>
	<p class="text-surface-400 mb-6">{m.docs_wiki_description()}</p>

	<div class="mb-6">
		<WikiSearch bind:value={query} />
	</div>

	{#if query.trim()}
		{#if results.length === 0}
			<div class="text-surface-400 flex items-center justify-center py-12">
				<p>{m.docs_no_results()}</p>
			</div>
		{:else}
			<div class="space-y-6">
				{#each [...resultsByCategory] as [category, entries] (category)}
					<div>
						<h2 class="text-surface-400 mb-2 text-sm font-semibold">{categoryLabel(category)}</h2>
						<div class="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3">
							{#each entries as entry (entry.key)}
								{@const link = entityLink(entry.category, entry.key)}
								{@const descriptor = descriptorFor(entry.category)}
								{@const record = descriptor.runtime()[entry.key] as
									| Record<string, unknown>
									| undefined}
								{@const icon = record ? (descriptor.icon?.(entry.key, record) ?? null) : null}
								{@const meta = record ? (descriptor.cardMeta?.(entry.key, record) ?? null) : null}
								<WikiCard href={link.href} name={entry.name} {icon} {meta} subtext={entry.key} />
							{/each}
						</div>
					</div>
				{/each}
			</div>
		{/if}
	{:else}
		<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
			{#each WIKI_CATEGORIES as category (category.id)}
				{@const meta = categoryMeta[category.id]}
				<a
					href={categoryHref(category.id)}
					class="group border-surface-800 hover:border-primary-500/50 hover:bg-surface-700 rounded-lg border p-4 transition-colors"
				>
					<div class="mb-2 flex items-center gap-2">
						<meta.icon class="text-primary-500 h-5 w-5" />
						<h2 class="text-lg font-semibold">{categoryLabel(category.id)}</h2>
					</div>
					<p class="text-surface-400 text-sm">{meta.description}</p>
					<p class="text-surface-500 mt-2 text-xs">{entriesByCategory[category.id].length}</p>
				</a>
			{/each}
		</div>
	{/if}
</div>
