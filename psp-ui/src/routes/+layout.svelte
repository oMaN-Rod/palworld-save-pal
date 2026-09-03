<script lang="ts">
	import '../app.css';
	import { Sidebar, PublicNav, TrayUnavailableBanner } from '$components/layout';
	import { Toast, Modal, Spinner, PalEditorOverlay, ResizeWarning } from '$components/ui';
	import { bootstrap } from '$lib/data/bootstrap';
	import { cornerArt, getAppState, getSocketState, theme, localeState } from '$states';
	import { goto } from '$app/navigation';
	import {
		isSaveRequiredRoute,
		isFullBleedRoute,
		isPublicShell,
		isCompatExemptRoute,
		isControlRoute
	} from '$lib/utils/shellRoutes';
	import { localizedPath, siteLocales } from '$lib/i18n/routingConfig.js';
	import { getDispatcher } from '$lib/ws/dispatcher';
	import { handlers } from '$lib/ws/handlers';
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { page } from '$app/state';
	import * as m from '$i18n/messages';
	import { send } from '$utils/websocketUtils';
	import { MessageType } from '$types';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import {
		setStoredSelectedPlayerUid,
		clearStoredSelectedPlayerUid,
		getStoredSessionId
	} from '$lib/utils/sessionPersistence';
	import { isWebBuild } from '$lib/utils/platform';
	import { syncLocaleToPath } from '$lib/i18n/appLocale';
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
	// The Linux first-run mode-select overlay renders with no shell chrome at
	// all. The `?path=` arm is a boot safety net (a deep link could still arrive
	// as `/?path=/mode-select`): without it the small-window overlay mounts
	// during that interim. searchParams is runtime-only: reading it during
	// prerender throws, so gate on `browser`.
	const controlShell = $derived(
		isControlRoute(page.url.pathname) ||
			(browser && isControlRoute(page.url.searchParams.get('path') ?? ''))
	);

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

	$effect(() => {
		syncLocaleToPath(page.url.pathname);
	});

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

		// Linux desktop shell: the editor finished bootstrapping — signal it so
		// it can reveal the hidden window instead of flashing a blank webview.
		// Skip the mode-select overlay, which has no editor window to reveal.
		if (PUBLIC_DESKTOP_MODE === 'true' && !controlShell) {
			send(MessageType.READY);
		}
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
		{#if publicShell && !isCompatExemptRoute(page.url.pathname)}
			<CompatBanner />
		{/if}
		<Modal>
			<div class="relative z-[1] flex h-screen w-full overflow-hidden">
				{#if publicShell}
					<PublicNav />
				{:else if !controlShell}
					<Sidebar />
				{/if}
				<div class="relative flex flex-1 flex-col overflow-hidden">
					<!-- Linux browser mode with no displayable tray icon: the shell's
					     Quit fallback. Renders nothing everywhere else (it self-queries
					     the shell via get_display_mode). -->
					<TrayUnavailableBanner />
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
	<!-- Sits under the z-[1] shell, above the body gradients. -->
	{#if cornerArt.current && !isLandingPath(page.url.pathname) && !controlShell}
		<div
			class="pointer-events-none fixed inset-0 z-0"
			style="background: url('/bg-corner.webp') no-repeat bottom right / 880px auto; opacity: 0.1;"
			aria-hidden="true"
		></div>
	{/if}
	<PalEditorOverlay />
	{#if !controlShell}
		<ResizeWarning />
	{/if}
{/if}
