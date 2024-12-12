<script lang="ts">
	import { Spinner } from '$components';
	import { getAppState } from '$states';

	const appState = getAppState();
</script>

<div class="flex h-full w-full flex-col items-center justify-center">
	<h2 class="h2 mb-8">🤖 Beep Boop, working on it!</h2>
	<Spinner size="size-32" />
	{#if appState.progressMessage}
		<span class="mt-2">{appState.progressMessage}</span>
	{/if}
</div>
