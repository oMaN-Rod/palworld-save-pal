<script lang="ts">
	import { tours } from '$docs/tours';
	import { tourService } from '$docs/tours/tourService.svelte';
	import { getAppState } from '$states';
	import { Play, AlertCircle } from 'lucide-svelte';
	import * as m from '$i18n/messages';
	import { Button } from '$components/ui';

	const appState = getAppState();
</script>

<div class="p-4">
	<h1 class="mb-2 text-2xl font-bold">{m.docs_tours()}</h1>
	<p class="text-surface-400 mb-6">{m.docs_tours_description()}</p>

	<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
		{#each tours as tour}
			{@const disabled = tour.requiresSaveFile && !appState.saveFile}
			<div class="border-surface-700 bg-surface-800 rounded-lg border p-4">
				<h3 class="text-lg font-semibold">{tour.title}</h3>
				<p class="text-surface-400 mt-1 text-sm">{tour.description}</p>

				{#if disabled}
					<div class="mt-3 flex items-center gap-1 text-xs text-amber-400">
						<AlertCircle class="h-3.5 w-3.5" />
						<span>{m.docs_tour_requires_save()}</span>
					</div>
				{/if}

				<Button
					variant="primary"
					size="sm"
					class="mt-3"
					{disabled}
					onclick={() => tourService.startTour(tour.id)}
				>
					<Play class="h-4 w-4" />
					{m.docs_start_tour()}
				</Button>
			</div>
		{/each}
	</div>
</div>
