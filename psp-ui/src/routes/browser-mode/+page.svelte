<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Spinner } from '$components/ui';
	import { getAppState } from '$states';
	import { send, sendAndWait } from '$utils/websocketUtils';
	import { MessageType } from '$types';
	import { PUBLIC_WS_URL, PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { onMount } from 'svelte';

	const isDesktop = PUBLIC_DESKTOP_MODE === 'true';
	const appState = getAppState();

	// PUBLIC_WS_URL is baked as "127.0.0.1:5174/ws" in desktop builds; the
	// user-facing URL shows localhost like the shell-open handler does.
	const hostPort = PUBLIC_WS_URL.replace(/\/ws\/?$/, '');
	const port = hostPort.split(':').pop() ?? '';
	const url = `http://${hostPort.replace('127.0.0.1', 'localhost')}`;

	type StepState = 'pending' | 'active' | 'done';
	let steps = $state<{ label: string; state: StepState }[]>([
		{ label: 'Starting backend', state: 'pending' },
		{ label: 'Connecting to backend', state: 'pending' },
		{ label: 'Verifying server version', state: 'pending' },
		{ label: 'Opening your browser', state: 'pending' }
	]);
	let ready = $state(false);
	let version = $state(appState.version ?? '');
	let copied = $state(false);
	let quitting = $state(false);
	let bootError = $state('');

	function setStep(index: number, state: StepState) {
		steps[index].state = state;
	}

	// The root layout opens the socket on mount; sendAndWait below parks until
	// that socket is OPEN, so the boot sequence is driven by plain awaits —
	// no reactive socket state involved.
	onMount(() => {
		if (!isDesktop) return;
		void bootSequence();
	});

	async function bootSequence() {
		try {
			// This page is served BY the backend, so step 0 is already proven.
			setStep(0, 'done');
			setStep(1, 'active');
			// The round-trip both proves the socket connected and fetches the
			// version — on a cold webview start the visible spinner time here
			// is the real connect wait.
			const response = await sendAndWait<string>(MessageType.GET_VERSION);
			version = typeof response === 'string' ? response : String(response ?? '');
			setStep(1, 'done');
			setStep(2, 'done');
			setStep(3, 'active');
			send(MessageType.OPEN_IN_BROWSER, hostPort);
			setStep(3, 'done');
			ready = true;
		} catch (error) {
			bootError = error instanceof Error ? error.message : String(error);
		}
	}

	function openInBrowser() {
		send(MessageType.OPEN_IN_BROWSER, hostPort);
	}

	async function copyUrl() {
		try {
			await navigator.clipboard.writeText(url);
			copied = true;
			setTimeout(() => (copied = false), 1500);
		} catch {
			// Clipboard can be denied in webviews; the URL is selectable text.
		}
	}

	function quit() {
		if (quitting) return;
		quitting = true;
		send(MessageType.SHUTDOWN, null);
		// Belt and suspenders: closing the window also stops the app when the
		// shutdown message could not be delivered (socket already dead).
		setTimeout(() => window.close(), 1500);
	}
</script>

<div class="flex h-full w-full flex-col items-center justify-center gap-5 p-6">
	{#if !isDesktop}
		<div class="max-w-sm text-center">
			<Icon icon="tabler:browser-off" size={40} class="text-surface-400 mx-auto mb-3" />
			<h2 class="h2 mb-2">Browser Mode control panel</h2>
			<p class="text-muted text-sm">
				This page belongs to the desktop app's browser-mode launcher. Start the browser-mode
				AppImage to use it.
			</p>
		</div>
	{:else if bootError}
		<div class="max-w-sm text-center">
			<Icon icon="tabler:alert-triangle" size={40} class="mx-auto mb-3 text-yellow-400" />
			<h2 class="h2 mb-2">Startup failed</h2>
			<p class="text-muted text-sm">{bootError}</p>
			<Button onclick={() => window.location.reload()} class="mt-4">Retry</Button>
		</div>
	{:else}
		<div class="flex items-center gap-3">
			<Icon icon="tabler:world-upload" size={28} class="text-primary-400" />
			<div>
				<h2 class="text-lg leading-tight font-bold">Palworld Save Pal</h2>
				<p class="text-muted text-xs tracking-wider uppercase">Browser Mode</p>
			</div>
		</div>

		<!-- Live boot sequence -->
		<div class="w-full max-w-xs space-y-2.5">
			{#each steps as step (step.label)}
				<div class="flex items-center gap-2.5">
					{#if step.state === 'done'}
						<Icon icon="tabler:circle-check" size={18} class="shrink-0 text-green-400" />
						<span class="text-surface-50 text-sm">{step.label}</span>
					{:else if step.state === 'active'}
						<Spinner size="size-4" />
						<span class="text-primary-300 text-sm font-medium">{step.label}</span>
					{:else}
						<Icon icon="tabler:circle-dashed" size={18} class="text-surface-600 shrink-0" />
						<span class="text-surface-500 text-sm">{step.label}</span>
					{/if}
				</div>
			{/each}
		</div>

		{#if ready}
			<div class="w-full max-w-xs space-y-3">
				<div class="border-surface-500/40 bg-surface-900/80 rounded-lg border p-3.5">
					<div class="flex items-center justify-between gap-2">
						<span class="text-muted text-[11px] font-semibold tracking-wider uppercase">
							{quitting ? 'shutting down' : 'ready'}
						</span>
						{#if !quitting}
							<span class="flex items-center gap-1.5 text-xs">
								<span class="relative flex size-2">
									<span
										class="absolute inline-flex h-full w-full animate-ping rounded-full bg-green-400 opacity-60"
									></span>
									<span class="relative inline-flex size-2 rounded-full bg-green-400"></span>
								</span>
								<span class="text-muted">live</span>
							</span>
						{/if}
					</div>
					<button
						type="button"
						onclick={openInBrowser}
						class="text-primary-300 hover:text-primary-200 mt-1 block w-full truncate text-left text-sm font-medium transition-colors"
						title="Open in browser"
					>
						{url}
					</button>
					<div class="text-muted mt-1 flex gap-3 text-xs">
						<span>port {port}</span>
						{#if version}<span>v{version}</span>{/if}
					</div>
				</div>

				<div class="grid grid-cols-3 gap-2">
					<Button onclick={openInBrowser} class="text-xs">Open</Button>
					<Button onclick={copyUrl} class="text-xs">
						{copied ? 'Copied!' : 'Copy URL'}
					</Button>
					<Button onclick={quit} disabled={quitting} class="text-xs" variant="secondary">
						{quitting ? 'Quitting…' : 'Quit'}
					</Button>
				</div>
				<p class="text-muted text-center text-[11px]">
					The editor runs in your browser — closing this window stops the server.
				</p>
			</div>
		{/if}
	{/if}
</div>
