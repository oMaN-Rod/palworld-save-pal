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
	import * as m from '$i18n/messages';

	const features = [
		{ icon: PawPrint, title: m.landing_feature_pals_title, body: m.landing_feature_pals_body },
		{ icon: User, title: m.landing_feature_players_title, body: m.landing_feature_players_body },
		{ icon: Package, title: m.landing_feature_inventory_title, body: m.landing_feature_inventory_body },
		{ icon: FlaskConical, title: m.landing_feature_tech_title, body: m.landing_feature_tech_body },
		{ icon: Blocks, title: m.landing_feature_blueprints_title, body: m.landing_feature_blueprints_body },
		{ icon: Users, title: m.landing_feature_guilds_title, body: m.landing_feature_guilds_body },
		{ icon: Bookmark, title: m.landing_feature_presets_title, body: m.landing_feature_presets_body },
		{ icon: Languages, title: m.landing_feature_names_title, body: m.landing_feature_names_body }
	];
</script>

<section class="relative w-full overflow-hidden px-4 py-16">
	<h2 class="h2 mb-8 text-center font-bold">{m.landing_features_heading()}</h2>
	<div class="mx-auto grid max-w-5xl grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-3">
		{#each features as f (f.title)}
			<div class="card flex gap-4 p-5">
				<f.icon class="text-primary-400 h-8 w-8 shrink-0" />
				<div>
					<h3 class="h4 font-semibold">{f.title()}</h3>
					<p class="text-surface-300 text-sm">{f.body()}</p>
				</div>
			</div>
		{/each}
	</div>
</section>
