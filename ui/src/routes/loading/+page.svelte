<script lang="ts">
	import { Spinner } from '$components';
	import { getSocketState } from '$states';
	import { MessageType } from '$types';

	const ws = getSocketState();

	let progressMessage = $state('');

	$effect(() => {
		if (ws.message && ws.message.type) {
			const { data, type } = ws.message;
			if (type !== MessageType.PROGRESS_MESSAGE) return;
			progressMessage = data as string;
			ws.clear(type);
		}
	});
</script>

<div class="flex h-full w-full flex-col items-center justify-center">
	<h2 class="h2 mb-8">🤖 Beep Boop, working on it!</h2>
	<Spinner size="size-32" />
	{#if progressMessage}
		<span class="mt-2">{progressMessage}</span>
	{/if}
</div>
