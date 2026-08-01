<script lang="ts">
	import { getAppState } from '$states';
	import { goto } from '$app/navigation';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { Hero, Features, Cta } from '$components/landing';
	import { restoreMostRecent, hasRecent } from '$lib/fs';
	import { send, pushProgressMessage } from '$lib/utils/websocketUtils';
	import { MessageType } from '$types';

	const appState = getAppState();
	const desktop = PUBLIC_DESKTOP_MODE === 'true';

	let resumeName = $state<string | null>(null);

	if (desktop) {
		// Desktop keeps its existing boot behavior into the editor.
		if (!appState.saveFile) goto('/file');
	} else if (appState.saveFile) {
		// Mid-session on web → go straight back to editing.
		goto('/edit');
	} else {
		hasRecent().then((r) => (resumeName = r?.worldName ?? null));
	}

	function openSave() {
		goto('/upload');
	}

	async function resume() {
		await goto('/loading');
		appState.resetState();
		pushProgressMessage('Restoring your last save...');
		const r = await restoreMostRecent((bytes) => send(MessageType.LOAD_ZIP_FILE, Array.from(bytes)));
		if (!r.restored) await goto('/upload');
	}
</script>

<svelte:head>
	<title>Palworld Save Pal — edit Palworld saves in your browser</title>
	<meta
		name="description"
		content="Open, edit and save Palworld saves entirely in your browser — pals, bases, blueprints and presets. Free and open source."
	/>
	<meta property="og:title" content="Palworld Save Pal" />
	<meta
		property="og:description"
		content="Edit Palworld saves in your browser — pals, bases, blueprints and presets."
	/>
	<meta property="og:type" content="website" />
</svelte:head>

{#if !desktop && !appState.saveFile}
	<main class="animate-fade-in flex w-full flex-col items-center">
		<Hero onOpen={openSave} onResume={resume} {resumeName} />
		<Features />
		<Cta />
	</main>
{/if}
