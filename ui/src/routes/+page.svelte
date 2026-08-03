<script lang="ts">
	import { getAppState, getToastState } from '$states';
	import { goto } from '$app/navigation';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { isWebBuild } from '$lib/utils/platform';
	import {
		Hero,
		MapAdvantage,
		Values,
		Features,
		HowItWorks,
		DesktopApp,
		Faq,
		Cta
	} from '$components/landing';
	import { restoreMostRecent, hasRecent } from '$lib/fs';
	import { send, pushProgressMessage } from '$lib/utils/websocketUtils';
	import { MessageType } from '$types';
	import { startSaveLoad } from '$lib/data/loadSave';
	import * as m from '$i18n/messages';

	const appState = getAppState();
	const toast = getToastState();
	const desktop = PUBLIC_DESKTOP_MODE === 'true';

	let resumeName = $state<string | null>(null);

	if (desktop) {
		if (!appState.saveFile) goto('/file');
	} else if (appState.saveFile) {
		goto('/edit');
	} else if (isWebBuild) {
		hasRecent().then((r) => (resumeName = r?.worldName ?? null));
	} else {
		goto('/upload');
	}

	async function resume() {
		await goto('/loading');
		appState.resetState();
		pushProgressMessage('Restoring your last save...');
		const r = await restoreMostRecent((bytes) => send(MessageType.LOAD_ZIP_FILE, Array.from(bytes)));
		if (!r.restored) {
			await goto('/upload');
			toast.add(
				r.needsPermission
					? 'Click "Open save folder" to reconnect your save folder.'
					: 'Could not restore the last save.',
				'Heads up',
				'warning'
			);
		}
	}
</script>

<svelte:head>
	<title>{m.landing_meta_title()}</title>
	<meta name="description" content={m.landing_meta_description()} />
	<meta property="og:title" content={m.landing_og_title()} />
	<meta property="og:description" content={m.landing_og_description()} />
	<meta property="og:type" content="website" />
</svelte:head>

{#if isWebBuild && !appState.saveFile}
	<main class="animate-fade-in flex w-full flex-col items-center">
		<Hero onLoad={startSaveLoad} onResume={resume} {resumeName} />
		<MapAdvantage />
		<Values />
		<Features />
		<HowItWorks />
		<DesktopApp />
		<Faq />
		<Cta />
	</main>
{/if}
