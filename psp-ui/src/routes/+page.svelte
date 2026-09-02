<script lang="ts">
	import { getAppState, getToastState } from '$states';
	import { browser } from '$app/environment';
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
	import { send, sendBytes, pushProgressMessage } from '$lib/utils/websocketUtils';
	import { MessageType } from '$types';
	import { startSaveLoad } from '$lib/data/loadSave';
	import * as m from '$i18n/messages';
	import { Seo, faqPageSchema, webApplicationSchema } from '$lib/components/seo';
	import { faqEntries } from '$components/landing/Faq.svelte';

	const appState = getAppState();
	const toast = getToastState();
	const desktop = PUBLIC_DESKTOP_MODE === 'true';

	let resumeName = $state<string | null>(null);

	if (browser) {
		// psp-server 307s deep links to /?path=<target> and the root layout
		// restores them — when one is pending, don't race it with the desktop
		// auto-redirect (that buried /browser-mode under /overview and tripped
		// the small-window overlay in the control window).
		const restorePath = new URLSearchParams(window.location.search).get('path');
		if (desktop) {
			if (!restorePath && !appState.saveFile) goto('/overview');
		} else if (appState.saveFile) {
			goto('/edit');
		} else if (isWebBuild) {
			hasRecent().then((r) => (resumeName = r?.worldName ?? null));
		} else {
			goto('/upload');
		}
	}

	async function resume() {
		await goto('/loading');
		appState.resetState();
		pushProgressMessage(m.upload_restoring());
		const r = await restoreMostRecent((bytes) => sendBytes(MessageType.LOAD_ZIP_FILE, bytes));
		if (!r.restored) {
			await goto('/upload');
			toast.add(
				r.needsPermission ? m.upload_reconnect_folder() : m.upload_restore_failed(),
				m.toast_heads_up(),
				'warning'
			);
		}
	}
</script>

<Seo
	pathname="/"
	title={m.landing_meta_title()}
	description={m.landing_meta_description()}
	ogTitle={m.landing_og_title()}
	ogDescription={m.landing_og_description()}
	structuredData={[webApplicationSchema(), faqPageSchema(faqEntries())]}
/>

{#if (isWebBuild || !browser) && !appState.saveFile}
	<main class="landing-page animate-fade-in flex w-full flex-col items-center">
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

<style>
	.landing-page {
		position: relative;
		overflow-x: clip;
		background:
			radial-gradient(
				900px 500px at 15% 15%,
				color-mix(in srgb, var(--color-primary-500) 6%, transparent),
				transparent 70%
			),
			radial-gradient(
				800px 500px at 85% 50%,
				color-mix(in srgb, var(--color-secondary-500) 5%, transparent),
				transparent 72%
			),
			radial-gradient(
				700px 400px at 50% 85%,
				color-mix(in srgb, var(--color-tertiary-500) 5%, transparent),
				transparent 70%
			);
		background-attachment: fixed;
	}
</style>
