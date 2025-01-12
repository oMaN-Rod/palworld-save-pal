<script lang="ts">
	import '../app.css';
	import { NavBar, Toast, Modal } from '$components';
	import { bootstrap } from '$lib/data';
	import { getSocketState } from '$states';
	import { goto } from '$app/navigation';
	import { getDispatcher } from '$lib/ws/dispatcher';
	import { handlers } from '$lib/ws/handlers';
	import { onMount } from 'svelte';

	const { children } = $props();
	const ws = getSocketState();
	const dispatcher = getDispatcher();

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
		<main class="flex-1 overflow-hidden">
			{@render children()}
		</main>
	</div>
</Modal>
