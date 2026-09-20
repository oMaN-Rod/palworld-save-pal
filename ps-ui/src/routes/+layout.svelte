<script lang="ts">
	import '../app.css';
	import { Sidebar, PublicNav, TitleBar, NavDrawer } from '$components/layout';
	import { Toast, Modal, Spinner, PalEditorOverlay } from '$components/ui';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { bootstrap } from '$lib/data/bootstrap';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { getWebSignalSession } from '$lib/signal/webSession';
	import { layout } from '$utils/layout.svelte';
	import { navDrawer } from '$states/navDrawer.svelte';
	import {
		cornerArt,
		getAppState,
		getModsState,
		getServerState,
		getSignalState,
		getSocketState,
		theme,
		localeState,
		ROSE_RED,
		rwbySkin
	} from '$states';
	import { goto } from '$app/navigation';
	import {
		isSaveRequiredRoute,
		isFullBleedRoute,
		isPipWindow,
		isPublicShell,
		isCompatExemptRoute
	} from '$lib/utils/shellRoutes';
	import { localizedPath, siteLocales } from '$lib/i18n/routingConfig.js';
	import { getDispatcher } from '$lib/ws/dispatcher';
	import { handlers } from '$lib/ws/handlers';
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { page } from '$app/state';
	import * as m from '$i18n/messages';
	import {
		setStoredSelectedPlayerUid,
		clearStoredSelectedPlayerUid,
		clearSessionPersistence,
		getStoredSessionId
	} from '$lib/utils/sessionPersistence';
	import { desktopChrome, isWebBuild } from '$lib/utils/platform';
	import { syncLocaleToPath } from '$lib/i18n/appLocale';
	import { browser } from '$app/environment';
	import { CompatBanner, UnsupportedBrowser } from '$components/compat';
	import { detectCapabilities, hardBlocked } from '$lib/utils/browserCapabilities';
	import { send } from '$lib/utils/websocketUtils';
	import { baseStructuresData } from '$lib/data';
	import { MessageType } from '$types';

	const { children } = $props();
	const ws = getSocketState();
	const dispatcher = getDispatcher();
	const appState = getAppState();
	const remoteMode = getRemoteMode();
	const remoteSession = browser && isWebBuild ? getWebSignalSession() : null;

	// Web build only: desktop and Docker ship a real backend and native file
	// access, so none of these browser limits apply there. The `browser` guard
	// matters because adapter-static prerenders in Node, where `Worker` is not a
	// global — detecting there would bake the block screen into shipped HTML.
	const blocked = browser && isWebBuild && hardBlocked(detectCapabilities());
	const publicShell = $derived(isPublicShell(isWebBuild, appState.saveFile, remoteMode.active));
	const pipWindow = $derived(browser && isPipWindow(page.url));
	const showTitleBar = $derived(browser && desktopChrome() && !pipWindow);

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
		document.body.classList.toggle('rwby-skin', rwbySkin.current);
	});

	// The title bar eats the top of the viewport, so every `100vh` in the app has
	// to subtract it. That has to be readable from anywhere, hence the document
	// element rather than a component — app.css defaults it to `0px` so the web
	// and Docker builds stay correct without this ever running.
	$effect(() => {
		document.documentElement.style.setProperty(
			'--titlebar-h',
			showTitleBar ? 'var(--titlebar-size)' : '0px'
		);
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
				appState.saveState().catch((error) => {
					console.error('Error auto-saving on unload:', error);
				});
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

	let signalSeeded = false;
	$effect(() => {
		if (isWebBuild || signalSeeded || !ws.connected) return;
		signalSeeded = true;
		getSignalState().refresh();
	});

	$effect(() => {
		const connected = ws.connected;
		const kind = remoteMode.active ? 'remote' : 'socket';
		getModsState().connectionChanged(connected, kind);
		getServerState().connectionChanged(connected, kind);
	});

	$effect(() => {
		const transport = remoteMode.active ? remoteMode.transport : null;
		getModsState().transportChanged(transport);
		getServerState().transportChanged(transport);
	});

	$effect(() => {
		if (!remoteSession) return;
		if (remoteMode.active && remoteSession.state === 'failed') {
			remoteMode.exit();
			if (appState.saveFile) {
				appState.resetState();
				baseStructuresData.reset();
				clearSessionPersistence();
			}
		}
	});

	let remoteReattachPrevConnected = remoteSession?.connected ?? false;
	$effect(() => {
		if (!remoteSession) return;
		const connected = remoteSession.connected;
		if (connected === remoteReattachPrevConnected) return;
		remoteReattachPrevConnected = connected;
		if (!connected || !remoteMode.active) return;
		remoteMode.transport?.resetFraming();
		const sessionId = remoteMode.transport?.lastSessionId;
		if (!sessionId) return;
		send(MessageType.REATTACH_SESSION, { session_id: sessionId });
	});

	onMount(async () => {
		if (blocked) return;
		ws.connect({ goto });

		try {
			await bootstrap();
		} catch (error) {
			console.error('Error during app bootstrap:', error);
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
		{#if publicShell && !pipWindow && !isCompatExemptRoute(page.url.pathname)}
			<CompatBanner />
		{/if}
		<Modal>
			<!-- The shell wrapper must not carry a `z-index`: that would trap the title
			     bar in a stacking context pinned below the modal overlays, leaving the
			     window undraggable and unclosable while a modal is open. The stacking
			     context lives on the inner wrapper instead, so `.title-bar` competes
			     directly in the root context and stays on top. -->
			<div class="relative flex h-screen w-full flex-col overflow-hidden">
				{#if showTitleBar}
					<TitleBar />
				{/if}
				<div class="relative z-[1] flex min-h-0 w-full flex-1 overflow-hidden">
					{#if !pipWindow}
						{#if publicShell}
							<PublicNav />
						{:else if layout.phone}
							<NavDrawer bind:open={navDrawer.open} onClose={() => (navDrawer.open = false)} />
						{:else}
							<Sidebar />
						{/if}
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
			</div>
		</Modal>
	{/key}
	{#if cornerArt.current && !rwbySkin.current && !pipWindow && !isLandingPath(page.url.pathname)}
		<div
			class="pointer-events-none fixed inset-0 z-0"
			style="background: url('/bg-corner.webp') no-repeat bottom right / 880px auto; opacity: 0.1;"
			aria-hidden="true"
		></div>
	{:else if rwbySkin.current && !pipWindow}
		<div
			class="pointer-events-none fixed inset-0 z-0 opacity-[0.14]"
			style="background: radial-gradient(ellipse at 70% 15%, rgb(238 52 80 / 0.08), transparent 60%);"
			aria-hidden="true"
		>
			<Icon icon="local:rwby-rose" size={560} color={ROSE_RED} class="absolute right-0 bottom-0" />
		</div>
	{/if}
	<PalEditorOverlay />
{/if}
