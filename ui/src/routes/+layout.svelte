<script lang="ts">
	import '../app.css';
	import { Sidebar, PublicNav } from '$components/layout';
	import { Toast, Modal, Spinner, PalEditorOverlay, ResizeWarning } from '$components/ui';
	import { bootstrap } from '$lib/data/bootstrap';
	import { getAppState, getSocketState, theme, localeState } from '$states';
	import { goto } from '$app/navigation';
	import { isSaveRequiredRoute, isFullBleedRoute, isPublicShell } from '$lib/utils/shellRoutes';
	import { localizedPath, siteLocales } from '$lib/i18n/routingConfig.js';
	import { getDispatcher } from '$lib/ws/dispatcher';
	import { handlers } from '$lib/ws/handlers';
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { page } from '$app/state';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import {
		setStoredSelectedPlayerUid,
		clearStoredSelectedPlayerUid,
		getStoredSessionId
	} from '$lib/utils/sessionPersistence';
	import { isWebBuild } from '$lib/utils/platform';
	import { browser } from '$app/environment';
	import { CompatBanner, UnsupportedBrowser } from '$components/compat';
	import { detectCapabilities, hardBlocked } from '$lib/utils/browserCapabilities';

	const { children } = $props();
	const ws = getSocketState();
	const dispatcher = getDispatcher();
	const appState = getAppState();

	// Web build only: desktop and Docker ship a real backend and native file
	// access, so none of these browser limits apply there. The `browser` guard
	// matters because adapter-static prerenders in Node, where `Worker` is not a
	// global — detecting there would bake the block screen into shipped HTML.
	const blocked = browser && isWebBuild && hardBlocked(detectCapabilities());
	const publicShell = $derived(isPublicShell(isWebBuild, appState.saveFile));

	// Every locale's landing root (`/`, `/de`, `/zh`, …) — the marketing page
	// stays clean of the ambient corner art.
	const landingPaths = new Set(['/', ...siteLocales.map((locale) => localizedPath('/', locale))]);
	function isLandingPath(pathname: string): boolean {
		return landingPaths.has(pathname.replace(/\/+$/, '') || '/');
	}

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

	// Only redirect when no session could possibly reattach — a stored session
	// id means bootstrap() may still populate saveFile, so let that race resolve
	// instead of bouncing a refreshing editor user off their save-only route.
	// Save-less visitors land on the upload page, matching where the sidebar
	// links already point.
	$effect(() => {
		if (publicShell && !getStoredSessionId() && isSaveRequiredRoute(page.url.pathname)) {
			goto('/upload');
		}
	});

	onMount(async () => {
		if (blocked) return;
		ws.connect({ goto });

		await bootstrap();
	});
</script>

{#if blocked}
	<UnsupportedBrowser />
{:else}
	<Toast position="bottom-center" transition={{ type: 'fly', params: { y: 300 } }} />
	<!-- Paraglide message accessors read module-scoped state, so nothing re-renders
	     on a locale change by itself. Keying the whole shell — not just the routed
	     page — is what re-translates the nav, banner and indicator too. -->
	{#key localeState.version}
		{#if publicShell}
			<CompatBanner />
		{/if}
		<Modal>
			<div class="relative z-[1] flex h-screen w-full overflow-hidden">
				{#if publicShell}
					<PublicNav />
				{:else}
					<Sidebar />
				{/if}
				<div class="relative flex flex-1 flex-col overflow-hidden">
					{#if appState.autoSave}
						<div class="auto-save-indicator" transition:fade>
							<span class="text-primary-400 text-sm font-bold">{m.syncing()}</span>
							<Spinner size="size-5" />
						</div>
					{/if}
					<div class="relative flex-1 overflow-hidden">
						{#key page.url.pathname}
							<main
								class="absolute inset-0 overflow-y-auto"
								class:public-shell-main={publicShell && !isFullBleedRoute(page.url.pathname)}
								transition:fade={{ duration: 150 }}
							>
								{@render children()}
							</main>
						{/key}
					</div>
				</div>
			</div>
		</Modal>
	{/key}
	<!-- Ambient corner art behind the app shell (PalSavTools-style): fixed,
	     non-interactive, sits under the z-[1] shell above the body gradients.
	     Skipped on the landing page (every locale's root) so its marketing
	     layout stays clean. -->
	{#if !isLandingPath(page.url.pathname)}
		<div
			class="pointer-events-none fixed inset-0 z-0"
			style="background: url('/bg-corner.webp') no-repeat bottom right / 880px auto; opacity: 0.5;"
			aria-hidden="true"
		></div>
	{/if}
	<PalEditorOverlay />
	<ResizeWarning />
{/if}
