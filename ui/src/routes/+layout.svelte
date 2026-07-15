<script lang="ts">
	import '../app.css';
	import { NavBar } from '$components/layout';
	import { Toast, Modal, Spinner, PalEditorOverlay } from '$components/ui';
	import { bootstrap } from '$lib/data/bootstrap';
	import { getAppState, getSocketState, theme } from '$states';
	import { goto } from '$app/navigation';
	import { getDispatcher } from '$lib/ws/dispatcher';
	import { handlers } from '$lib/ws/handlers';
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { page } from '$app/state';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import {
		setStoredSelectedPlayerUid,
		clearStoredSelectedPlayerUid
	} from '$lib/utils/sessionPersistence';

	const { children } = $props();
	const ws = getSocketState();
	const dispatcher = getDispatcher();
	const appState = getAppState();

	handlers.forEach((handler) => {
		dispatcher.register(handler);
	});

	// Keep the <body data-theme> attribute in sync with the persisted theme so
	// switching themes swaps the active color palette (client-side only).
	$effect(() => {
		document.body.dataset.theme = theme.current;
	});

	// Mirror the selected player to sessionStorage so a refresh can re-select it.
	$effect(() => {
		if (appState.selectedPlayerUid) {
			setStoredSelectedPlayerUid(appState.selectedPlayerUid);
		} else {
			clearStoredSelectedPlayerUid();
		}
	});

	// Best-effort autosave flush on refresh/close; no prompt, fire-and-forget.
	$effect(() => {
		function handleBeforeUnload(): void {
			if (appState.saveFile) {
				appState.saveState();
			}
		}
		window.addEventListener('beforeunload', handleBeforeUnload);
		return () => window.removeEventListener('beforeunload', handleBeforeUnload);
	});

	onMount(async () => {
		ws.connect({ goto });

		await bootstrap();
	});
</script>

<Toast position="bottom-center" transition={{ type: 'fly', params: { y: 300 } }} />
<Modal>
	<div class="flex h-screen w-full overflow-hidden">
		<NavBar />
		{#if appState.autoSave}
			<div class="auto-save-indicator" transition:fade>
				<span class="text-primary-400 text-sm font-bold">{m.syncing()}</span>
				<Spinner size="size-5" />
			</div>
		{/if}
		<div class="relative flex-1 overflow-hidden">
			{#key page.url.pathname}
				<main class="absolute inset-0 overflow-y-auto" transition:fade={{ duration: 150 }}>
					{@render children()}
				</main>
			{/key}
		</div>
	</div>
</Modal>
<PalEditorOverlay />
