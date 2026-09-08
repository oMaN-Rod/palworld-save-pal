<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { goto } from '$app/navigation';
	import { getLocale } from '$i18n/runtime';
	import * as m from '$i18n/messages';
	import { Seo } from '$lib/components/seo';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Card, Input } from '$components/ui';
	import { SidebarDetail } from '$components/layout';
	import { localizedPath } from '$lib/i18n/routingConfig.js';
	import { isWebBuild } from '$lib/utils/platform';
	import { parsePairingFragment } from '$lib/signal/session.svelte';
	import { getWebSignalSession, getStoredDesktop } from '$lib/signal/webSession';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { clearStoredDesktop, type StoredDesktop } from '$lib/signal/credentialStore';
	import { normalizeCode } from '$lib/signal/crypto';
	import { getLiveActors } from '$lib/data/liveActors.svelte';
	import { getAppState } from '$states';
	import DesktopSignalPanel from '$lib/components/signal/DesktopSignalPanel.svelte';
	import SignalRwbyEmblem from '$lib/components/signal/SignalRwbyEmblem.svelte';
	import RemoteSavePicker from '$lib/components/signal/RemoteSavePicker.svelte';
	import { type PickerSection } from '$lib/components/signal/pickerModel';

	const session = getWebSignalSession();
	const remoteMode = getRemoteMode();
	const appState = getAppState();

	const exitBlocked = $derived(remoteMode.active && !!appState.saveFile);

	type ConnectAttempt = { kind: 'code'; code: string } | { kind: 'stored'; desktop: StoredDesktop };

	let codeInput = $state('');
	let activeSection = $state<PickerSection>('steam');
	let lastAttempt = $state<ConnectAttempt | null>(null);
	let pairingDifferent = $state(false);
	let storeVersion = $state(0);
	let sourceStatus = $state<{ health?: string; actorCount?: number } | null>(null);
	let statusFailed = $state(false);

	const storedDesktop = $derived.by(() => {
		void storeVersion;
		if (!isWebBuild) return null;
		if (session.state !== 'idle' && session.state !== 'reconnecting') return null;
		return getStoredDesktop();
	});

	const connectedDesktopName = $derived.by(() => {
		void session.state;
		return getStoredDesktop()?.desktopName ?? null;
	});

	const HEALTH_LABELS: Record<string, () => string> = {
		idle: m.signal_health_idle,
		waiting: m.signal_health_waiting,
		auth: m.signal_health_auth,
		down: m.signal_health_down,
		stale: m.signal_health_stale,
		ok: m.signal_health_ok
	};

	function healthLabel(health: unknown): string {
		if (typeof health === 'string' && health in HEALTH_LABELS) return HEALTH_LABELS[health]();
		return typeof health === 'string' ? health : '';
	}

	function attemptConnect(code: string) {
		const normalized = normalizeCode(code);
		if (!normalized) return;
		lastAttempt = { kind: 'code', code: normalized };
		pairingDifferent = false;
		void session.connect(normalized);
	}

	function attemptConnectStored(desktop: StoredDesktop) {
		lastAttempt = { kind: 'stored', desktop };
		pairingDifferent = false;
		void session.connectStored(desktop);
	}

	function handleSubmit(event: SubmitEvent) {
		event.preventDefault();
		attemptConnect(codeInput);
	}

	function handleRetry() {
		if (!lastAttempt) return;
		if (lastAttempt.kind === 'code') attemptConnect(lastAttempt.code);
		else attemptConnectStored(lastAttempt.desktop);
	}

	function handleForgetDesktop() {
		clearStoredDesktop();
		storeVersion++;
		if (session.state === 'failed') session.disconnect();
	}

	function handleBackToRemembered() {
		session.disconnect();
	}

	function openLiveMap() {
		goto(`${localizedPath('/map', getLocale())}?live=1`);
	}

	function handleDisconnect() {
		if (remoteMode.active) remoteMode.exit();
		session.disconnect();
		getLiveActors().clear();
	}

	function toggleRemoteMode() {
		if (remoteMode.active) {
			if (exitBlocked) return;
			remoteMode.exit();
			return;
		}
		try {
			remoteMode.enter();
		} catch {
		}
	}

	const pickerSections: { id: PickerSection; icon: string; label: () => string }[] = [
		{ id: 'steam', icon: 'tabler:brand-steam', label: () => m.remote_saves_steam() },
		{ id: 'gamepass', icon: 'tabler:device-gamepad', label: () => m.remote_saves_gamepass() },
		{ id: 'server', icon: 'tabler:server', label: () => m.remote_saves_servers() },
		{ id: 'browse', icon: 'tabler:folder-search', label: () => m.remote_saves_browse() }
	];

	onMount(() => {
		if (!isWebBuild) return;
		const pairing = window as unknown as { __pspSignalPairing?: string };
		const captured = pairing.__pspSignalPairing;
		delete pairing.__pspSignalPairing;
		const code = captured ? parsePairingFragment(captured) : null;
		if (code) attemptConnect(code);
	});

	$effect(() => {
		if (session.state !== 'connected') return;
		sourceStatus = null;
		statusFailed = false;
		session
			.sendAndWait('status')
			.then((data) => {
				sourceStatus = (data ?? null) as { health?: string; actorCount?: number } | null;
			})
			.catch(() => {
				statusFailed = true;
			});
	});
</script>

<Seo
	pathname="/signal"
	title={m.signal_meta_title()}
	description={m.signal_meta_description()}
	noindex
/>

{#if !isWebBuild}
	<DesktopSignalPanel />
{:else if session.state === 'connected'}
	<SidebarDetail class="animate-fade-in" detailActive={remoteMode.active}>
		{#snippet sidebar()}
			<Card>
				<div class="flex flex-col gap-5">
					<div class="flex flex-col items-center gap-1 text-center">
						<Icon icon="tabler:circle-check" size={32} class="text-success-400" />
						<h2 class="h4 font-semibold">{m.signal_connected_heading()}</h2>
						<p class="text-surface-200 flex items-center gap-1.5 text-sm font-medium">
							<Icon icon="tabler:device-desktop" size={16} class="text-surface-400" />
							{connectedDesktopName ?? m.signal_desktop_fallback_name()}
						</p>
						{#if sourceStatus}
							<p class="text-surface-300 text-sm">
								{m.signal_source_health({ health: healthLabel(sourceStatus.health) })}
							</p>
							<p class="text-surface-300 text-sm">
								{m.signal_source_actors({ count: sourceStatus.actorCount ?? 0 })}
							</p>
						{:else if statusFailed}
							<p class="text-surface-400 text-xs">{m.signal_status_unavailable()}</p>
						{/if}
						{#if session.viaRelay}
							<p class="text-surface-400 text-xs">{m.signal_via_relay()}</p>
						{/if}
					</div>

					<div class="flex flex-col gap-2">
						<Button variant="primary" onclick={openLiveMap}>{m.signal_open_live_map()}</Button>
						{#if remoteMode.active}
							<Button variant="primary" onclick={() => goto('/servers')}>Servers</Button>
						{/if}
						<Button
							variant="neutral"
							onclick={toggleRemoteMode}
							disabled={exitBlocked}
							title={exitBlocked ? m.signal_remote_mode_exit_blocked_hint() : undefined}
						>
							{remoteMode.active ? m.signal_remote_mode_exit() : m.signal_remote_mode_enter()}
						</Button>
						<Button variant="ghost" onclick={handleDisconnect}>{m.signal_disconnect()}</Button>
					</div>

					{#if remoteMode.active}
						<div class="flex flex-col gap-1" transition:fade={{ duration: 200 }}>
							<p class="text-surface-400 px-1 text-xs font-semibold tracking-wide uppercase">
								{m.remote_saves_title()}
							</p>
							{#each pickerSections as sectionOption (sectionOption.id)}
								<button
									type="button"
									class="flex w-full items-center gap-2 rounded-sm p-2 text-left text-sm transition-colors {activeSection ===
									sectionOption.id
										? 'bg-secondary-500/25 text-surface-50'
										: 'text-surface-300 hover:bg-surface-800'}"
									onclick={() => (activeSection = sectionOption.id)}
								>
									<Icon icon={sectionOption.icon} size={16} class="shrink-0" />
									<span>{sectionOption.label()}</span>
								</button>
							{/each}
						</div>
					{/if}
				</div>
			</Card>
		{/snippet}
		{#snippet detail()}
			<RemoteSavePicker section={activeSection} />
		{/snippet}
	</SidebarDetail>
{:else}
	<div class="mx-auto flex w-full max-w-md flex-col items-center gap-4 p-4">
		<div class="flex items-center justify-center gap-1.5">
			<h1 class="h3 text-center font-bold">{m.signal()}</h1>
			<SignalRwbyEmblem />
		</div>

		{#if session.state === 'connecting'}
			<Card>
				<div class="flex flex-col items-center gap-3 py-4">
					<Icon icon="svg-spinners:180-ring-with-bg" size={48} class="text-secondary-400" />
					<p class="text-surface-300 text-sm">{m.signal_connecting()}</p>
				</div>
			</Card>
		{:else if session.state === 'reconnecting'}
			<Card>
				<div class="flex flex-col items-center gap-3 py-4">
					<Icon icon="svg-spinners:180-ring-with-bg" size={48} class="text-secondary-400" />
					<p class="text-surface-300 text-center text-sm">
						{m.signal_reconnecting({
							name: storedDesktop?.desktopName ?? m.signal_desktop_fallback_name(),
							attempt: session.reconnectAttempt
						})}
					</p>
					<Button variant="neutral" onclick={() => session.cancelReconnect()}>{m.cancel()}</Button>
				</div>
			</Card>
		{:else if session.state === 'failed'}
			<Card>
				<div class="flex flex-col items-center gap-3 py-2">
					<Icon icon="tabler:alert-triangle" size={32} class="text-warning-400" />
					<h2 class="h4 font-semibold">{m.signal_failed_heading()}</h2>
					<p class="text-surface-300 text-center text-sm">{session.failureHint}</p>
					{#if session.failure === 'direct-connect-failed'}
						<p class="text-surface-400 text-center text-xs">{m.signal_turn_note()}</p>
					{/if}
					<div class="flex flex-wrap items-center justify-center gap-2">
						<Button variant="neutral" onclick={handleRetry}>{m.signal_retry()}</Button>
						{#if lastAttempt?.kind === 'stored'}
							<Button variant="ghost" onclick={handleForgetDesktop}>
								{m.signal_forget_desktop()}
							</Button>
						{/if}
					</div>
					{#if lastAttempt?.kind === 'stored'}
						<button
							type="button"
							class="text-surface-400 hover:text-surface-200 text-xs underline"
							onclick={handleBackToRemembered}
						>
							{m.signal_back_to_desktop()}
						</button>
					{/if}
				</div>
			</Card>
		{:else if storedDesktop && !pairingDifferent}
			<Card>
				<div class="flex flex-col items-center gap-3 py-2">
					<Icon icon="tabler:device-desktop" size={32} class="text-secondary-400" />
					<h2 class="h4 text-center font-semibold">{storedDesktop.desktopName}</h2>
					<p class="text-surface-300 text-center text-sm">{m.signal_remembered_intro()}</p>
					<div class="flex flex-wrap items-center justify-center gap-2">
						<Button variant="primary" onclick={() => attemptConnectStored(storedDesktop)}>
							{m.signal_connect()}
						</Button>
						<Button variant="ghost" onclick={handleForgetDesktop}>
							{m.signal_forget_desktop()}
						</Button>
					</div>
					<button
						type="button"
						class="text-surface-400 hover:text-surface-200 text-xs underline"
						onclick={() => (pairingDifferent = true)}
					>
						{m.signal_pair_different()}
					</button>
				</div>
			</Card>
		{:else}
			<Card>
				<form class="flex flex-col gap-3" onsubmit={handleSubmit}>
					<p class="text-surface-300 text-sm">{m.signal_intro()}</p>
					<Input
						label={m.signal_code_label()}
						placeholder={m.signal_code_placeholder()}
						bind:value={codeInput}
						autocomplete="off"
					/>
					<Button type="submit" variant="primary" disabled={!normalizeCode(codeInput)}>
						{m.signal_connect()}
					</Button>
					{#if storedDesktop}
						<button
							type="button"
							class="text-surface-400 hover:text-surface-200 self-center text-xs underline"
							onclick={() => (pairingDifferent = false)}
						>
							{m.signal_pair_different_back()}
						</button>
					{/if}
				</form>
			</Card>
		{/if}
	</div>
{/if}
