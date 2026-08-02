<script lang="ts">
	import {
		PawPrint,
		User,
		Package,
		FlaskConical,
		Blocks,
		Users,
		Bookmark,
		Languages
	} from 'lucide-svelte';
	import SectionGlow from './SectionGlow.svelte';

	const features = [
		{ icon: PawPrint, title: 'Pals', body: 'Level, IVs, passives, moves, souls, rank, gender. Add, clone or delete.' },
		{ icon: User, title: 'Players', body: 'Level, status points, HP, condition, name and more.' },
		{ icon: Package, title: 'Inventory and storage', body: 'Items, materials, ammo, stacks and base storage.' },
		{ icon: FlaskConical, title: 'Technology and recipes', body: 'Tech points, boss points and recipe unlocks.' },
		{ icon: Blocks, title: 'Blueprints', body: 'Capture base structures and re-place them across worlds.' },
		{ icon: Users, title: 'Guilds and GPS', body: 'Manage guilds, transfer and clone pals between saves.' },
		{ icon: Bookmark, title: 'Presets', body: 'Save and reapply loadouts, stored right in your browser.' },
		{ icon: Languages, title: 'Real in-game names', body: 'Localized names across 16 languages.' }
	];
</script>

<section class="relative w-full overflow-hidden px-4 py-16">
	<SectionGlow />
	<h2 class="h2 mb-8 text-center font-bold">Everything you can edit</h2>
	<div class="mx-auto grid max-w-5xl grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-3">
		{#each features as f (f.title)}
			<div class="card flex gap-4 p-5">
				<f.icon class="text-primary-400 h-8 w-8 shrink-0" />
				<div>
					<h3 class="h4 font-semibold">{f.title}</h3>
					<p class="text-surface-300 text-sm">{f.body}</p>
				</div>
			</div>
		{/each}
	</div>
</section>
