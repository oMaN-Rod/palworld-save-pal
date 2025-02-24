<script lang="ts">
	import '../app.css';
	import { NavBar, Toast, Modal, Spinner } from '$components';
	import { bootstrap } from '$lib/data';
	import { getAppState, getSocketState } from '$states';
	import { goto } from '$app/navigation';
	import { getDispatcher } from '$lib/ws/dispatcher';
	import { handlers } from '$lib/ws/handlers';
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';

	const { children } = $props();
	const ws = getSocketState();
	const dispatcher = getDispatcher();
	const appState = getAppState();

	handlers.forEach((handler) => {
		dispatcher.register(handler);
	});

	$effect(() => {
		ws.connect({ goto });
	});

	onMount(async () => {
		await bootstrap();
	});
</script>

<Toast position="bottom-center" transition={{ type: 'fly', params: { y: 300 } }} />
<Modal>
	<div class="flex h-screen w-full overflow-hidden">
		<NavBar />
		{#if appState.autoSave}
			<div class="absolute right-2 top-1 flex flex-shrink-0 flex-row" transition:fade>
				<div class="flex items-center space-x-2 rounded-full p-3">
					<span class="text-lg font-bold">Syncing</span>
					<Spinner size="size-6" />
				</div>
			</div>
		{/if}
		<main class="flex-1 overflow-hidden">
			{@render children()}
		</main>
	</div>
</Modal>
