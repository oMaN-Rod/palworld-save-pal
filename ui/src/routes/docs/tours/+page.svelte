<script lang="ts">
	import { tours } from '$docs/tours';
	import { tourService } from '$docs/tours/tourService.svelte';
	import { getAppState } from '$states';
	import { Play, AlertCircle } from 'lucide-svelte';
	import * as m from '$i18n/messages';

	const appState = getAppState();
</script>

<div class="p-4">
	<h1 class="mb-2 text-2xl font-bold">{m.docs_tours()}</h1>
	<p class="mb-6 text-surface-400">{m.docs_tours_description()}</p>

	<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
		{#each tours as tour}
			{@const disabled = tour.requiresSaveFile && !appState.saveFile}
			<div class="rounded-lg border border-surface-700 bg-surface-800 p-4">
				<h3 class="text-lg font-semibold">{tour.title}</h3>
				<p class="mt-1 text-sm text-surface-400">{tour.description}</p>

				{#if disabled}
					<div class="mt-3 flex items-center gap-1 text-xs text-amber-400">
						<AlertCircle class="h-3.5 w-3.5" />
						<span>{m.docs_tour_requires_save()}</span>
					</div>
				{/if}

				<button
					class="mt-3 inline-flex items-center gap-2 rounded-md bg-primary-500 px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-primary-600 disabled:cursor-not-allowed disabled:opacity-50"
					{disabled}
					onclick={() => tourService.startTour(tour.id)}
				>
					<Play class="h-4 w-4" />
					{m.docs_start_tour()}
				</button>
			</div>
		{/each}
	</div>
</div>
