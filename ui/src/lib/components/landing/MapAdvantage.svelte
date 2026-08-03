<script lang="ts">
	import { Boxes, MapPin, MousePointerClick } from 'lucide-svelte';
	import SectionGlow from './SectionGlow.svelte';
	import * as m from '$i18n/messages';

	const points = [
		{ icon: Boxes, text: m.landing_map_point_terrain },
		{ icon: MapPin, text: m.landing_map_point_pins },
		{ icon: MousePointerClick, text: m.landing_map_point_click }
	];
</script>

<section class="relative w-full overflow-hidden px-4 py-16 text-center">
	<SectionGlow />
	<h2 class="h2 mx-auto max-w-3xl font-bold">
		{m.landing_map_heading()}
	</h2>
	<p class="text-surface-300 mx-auto mt-4 max-w-2xl">
		{m.landing_map_body()}
	</p>
	<div class="mx-auto mt-8 grid max-w-3xl grid-cols-1 gap-4 sm:grid-cols-3">
		{#each points as p, i (i)}
			<div class="flex flex-col items-center gap-2">
				<p.icon class="text-primary-400 h-7 w-7" />
				<span class="text-surface-200 text-sm">{p.text()}</span>
			</div>
		{/each}
	</div>
	<p class="text-surface-400 mt-8 text-sm">{m.landing_map_caption()}</p>
</section>
