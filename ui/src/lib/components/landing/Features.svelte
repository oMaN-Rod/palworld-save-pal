<script lang="ts">
	import { Card } from '$components/ui';
	import { MapPinned, PawPrint, Blocks, BookmarkCheck } from 'lucide-svelte';

	const features = [
		{
			icon: MapPinned,
			title: 'World map',
			body: 'Browse bases, fast-travel points and points of interest in 3D.'
		},
		{
			icon: PawPrint,
			title: 'Pals & bulk edit',
			body: 'Tweak stats, moves and IVs — one pal or many at once.'
		},
		{
			icon: Blocks,
			title: 'Blueprints',
			body: 'Capture and re-place base structures across worlds.'
		},
		{
			icon: BookmarkCheck,
			title: 'Presets',
			body: 'Save and reapply loadouts, stored right in your browser.'
		}
	];
</script>

<section class="mx-auto grid w-full max-w-4xl grid-cols-1 gap-6 px-4 py-12 sm:grid-cols-2">
	{#each features as f (f.title)}
		<Card class="flex gap-4">
			<f.icon class="text-primary-400 h-8 w-8 shrink-0" />
			<div>
				<h3 class="h4 font-semibold">{f.title}</h3>
				<p class="text-surface-300 text-sm">{f.body}</p>
			</div>
		</Card>
	{/each}
</section>
