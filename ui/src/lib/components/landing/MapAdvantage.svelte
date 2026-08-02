<script lang="ts">
	import { Boxes, MapPin, MousePointerClick } from 'lucide-svelte';
	import SectionGlow from './SectionGlow.svelte';

	const points = [
		{ icon: Boxes, text: 'Real 3D terrain, not a flat picture.' },
		{ icon: MapPin, text: 'Your bases and pals, pinned on the map.' },
		{ icon: MousePointerClick, text: 'Click a base to jump straight to it.' }
	];
</script>

<section class="relative w-full overflow-hidden px-4 py-16 text-center">
	<SectionGlow />
	<h2 class="h2 mx-auto max-w-3xl font-bold">
		The first Palworld save editor with a full 3D world map
	</h2>
	<p class="text-surface-300 mx-auto mt-4 max-w-2xl">
		Orbit the Palpagos Islands in real 3D, right in your browser. Tilt the world, zoom into your
		bases, and see your structures rendered in three dimensions. No other editor or site does this.
	</p>
	<div class="mx-auto mt-8 grid max-w-3xl grid-cols-1 gap-4 sm:grid-cols-3">
		{#each points as p (p.text)}
			<div class="flex flex-col items-center gap-2">
				<p.icon class="text-primary-400 h-7 w-7" />
				<span class="text-surface-200 text-sm">{p.text}</span>
			</div>
		{/each}
	</div>
	<p class="text-surface-400 mt-8 text-sm">The map above is live, rendered right in your browser.</p>
</section>
