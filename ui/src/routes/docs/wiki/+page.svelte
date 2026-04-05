<script lang="ts">
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import {
		Egg,
		Package,
		Building,
		Swords,
		Shield,
		FlaskConical,
		Flame,
		Hammer
	} from 'lucide-svelte';

	const categories = [
		{
			label: c.pal,
			href: '/docs/wiki/pals',
			icon: Egg,
			description: 'Stats, elements, skills, and work suitabilities for all Pals.'
		},
		{
			label: c.item,
			href: '/docs/wiki/items',
			icon: Package,
			description: 'All items including weapons, armor, consumables, and materials.'
		},
		{
			label: 'Buildings',
			href: '/docs/wiki/buildings',
			icon: Building,
			description: 'Building recipes, materials, and stats.'
		},
		{
			label: m.active_skill({ count: 2 }),
			href: '/docs/wiki/active-skills',
			icon: Swords,
			description: 'Combat skills with element types, power, and cooldowns.'
		},
		{
			label: m.passive_skill({ count: 2 }),
			href: '/docs/wiki/passive-skills',
			icon: Shield,
			description: 'Passive abilities and their stat effects.'
		},
		{
			label: m.technology({ count: 2 }),
			href: '/docs/wiki/technologies',
			icon: FlaskConical,
			description: 'Technology tree, unlock requirements, and costs.'
		},
		{
			label: 'Elements',
			href: '/docs/wiki/elements',
			icon: Flame,
			description: 'Element types and their properties.'
		},
		{
			label: 'Work Suitability',
			href: '/docs/wiki/work-suitability',
			icon: Hammer,
			description: 'Work types and which Pals excel at each.'
		}
	];
</script>

<div>
	<h1 class="mb-2 text-2xl font-bold">{m.docs_wiki()}</h1>
	<p class="text-surface-400 mb-6">{m.docs_wiki_description()}</p>

	<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
		{#each categories as cat}
			<a
				href={cat.href}
				class="group border-surface-800 hover:border-primary-500/50 hover:bg-surface-700 rounded-lg border p-4 transition-colors"
			>
				<div class="mb-2 flex items-center gap-2">
					<cat.icon class="text-primary-500 h-5 w-5" />
					<h2 class="text-lg font-semibold">{cat.label}</h2>
				</div>
				<p class="text-surface-400 text-sm">{cat.description}</p>
			</a>
		{/each}
	</div>
</div>
