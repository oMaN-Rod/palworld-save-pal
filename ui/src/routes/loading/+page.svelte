<script lang="ts">
	import { onMount } from 'svelte';
	import { Spinner } from '$components';
	import { getAppState } from '$states';
	import Stopwatch from '$components/ui/stopwatch/Stopwatch.svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	const appState = getAppState();

	let elapsed = $state(0);
	let intervalId: ReturnType<typeof setInterval> | null = null;

	onMount(() => {
		intervalId = setInterval(() => {
			elapsed += 1;
		}, 1000);

		return () => {
			if (intervalId) {
				clearInterval(intervalId);
			}
		};
	});
</script>

<div class="flex h-full w-full flex-col items-center justify-center">
	<h2 class="h2 mb-8">🤖 {m.working_on_it()}</h2>
	<Spinner size="size-32" />
	{#if appState.progressMessage}
		<span class="mt-2">{appState.progressMessage}</span>
	{/if}
	<Stopwatch bind:seconds={elapsed} />
</div>
