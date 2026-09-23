<script lang="ts">
	import { goto } from '$app/navigation';
	import { renderSVG } from 'uqr';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button } from '$components/ui';
	import * as m from '$i18n/messages';
	import { getSignalState } from '$states';
	import SignalArmedToggle from './SignalArmedToggle.svelte';
	import SignalDevicePane from './SignalDevicePane.svelte';
	import SignalRwbyEmblem from './SignalRwbyEmblem.svelte';
	import SignalSourcePopover from './SignalSourcePopover.svelte';
	import SignalStatusChips from './SignalStatusChips.svelte';

	const signalState = getSignalState();

	let pairingBusy = $state(false);
	let pairingError = $state<string | null>(null);
	let deviceError = $state<string | null>(null);
	let copyFeedback = $state(false);
	let now = $state(Date.now());

	const status = $derived(signalState.status);
	const pairing = $derived(status?.pairing ?? 'off');
	const devices = $derived(signalState.devices);

	const showsPairing = $derived(
		pairing === 'waiting' || pairing === 'connected' || devices.length === 0
	);

	const remainingMs = $derived(status?.expiresAtMs ? status.expiresAtMs - now : 0);
	const remainingLabel = $derived.by(() => {
		if (remainingMs <= 0) return m.signal_pairing_expired();
		const totalSeconds = Math.ceil(remainingMs / 1000);
		const minutes = Math.floor(totalSeconds / 60);
		const seconds = totalSeconds % 60;
		return m.signal_pairing_expires_in({
			time: `${minutes}:${seconds.toString().padStart(2, '0')}`
		});
	});

	const qrSvg = $derived(signalState.url ? renderSVG(signalState.url, { border: 1 }) : null);

	$effect(() => {
		signalState.refresh();
		const interval = setInterval(() => signalState.refresh(), 3000);
		return () => clearInterval(interval);
	});

	$effect(() => {
		void refreshDevices();
		const interval = setInterval(() => void refreshDevices(), 3000);
		return () => clearInterval(interval);
	});

	$effect(() => {
		if (pairing !== 'waiting') return;
		const interval = setInterval(() => (now = Date.now()), 1000);
		return () => clearInterval(interval);
	});

	async function refreshDevices() {
		try {
			await signalState.listDevices();
			deviceError = null;
		} catch (error) {
			deviceError = error instanceof Error ? error.message : String(error);
		}
	}

	async function startPairing() {
		pairingBusy = true;
		pairingError = null;
		try {
			await signalState.startPairing();
		} catch (error) {
			pairingError = error instanceof Error ? error.message : String(error);
		} finally {
			pairingBusy = false;
		}
	}

	async function stopPairing() {
		pairingBusy = true;
		try {
			await signalState.stopPairing();
		} catch (error) {
			pairingError = error instanceof Error ? error.message : String(error);
		} finally {
			pairingBusy = false;
		}
	}

	async function copyCode() {
		if (!status?.code) return;
		try {
			await navigator.clipboard.writeText(status.code);
			copyFeedback = true;
			setTimeout(() => (copyFeedback = false), 1500);
		} catch {
		}
	}

	function openLiveMap() {
		goto('/map?live=1');
	}
</script>

<div id="signal-panel" class="flex h-full w-full flex-col gap-4 p-4">
	<div class="flex flex-wrap items-center gap-x-4 gap-y-2">
		<div class="shrink-0">
			<div class="flex items-center gap-1.5">
				<h1 class="h3 font-bold">{m.signal()}</h1>
				<SignalRwbyEmblem />
			</div>
			<p class="text-surface-400 text-sm">{m.signal_desktop_subheading()}</p>
		</div>

		{#if status}
			<SignalStatusChips {status} />
		{/if}

		<div class="ml-auto flex items-center gap-3">
			<a class="tap-target text-primary-400 hover:text-primary-300 text-sm" href="/docs/guides/remote-access">
				{m.signal_how_it_works_link()}
			</a>
			<SignalSourcePopover />
		</div>
	</div>

	{#if deviceError}
		<p class="text-error-400 text-sm">{deviceError}</p>
	{/if}

	<div class="min-h-0 flex-1">
		{#if showsPairing}
			<div class="flex h-full flex-col items-center justify-center gap-4 text-center">
				{#if pairing === 'connected'}
					<Icon icon="tabler:circle-check" size={40} class="text-success-400" />
					<span class="text-lg font-semibold">{m.signal_connected_heading()}</span>
					<div class="flex gap-2">
						<Button variant="secondary" onclick={openLiveMap}>{m.signal_open_live_map()}</Button>
						<Button variant="neutral" disabled={pairingBusy} onclick={stopPairing}>
							{m.signal_pairing_stop()}
						</Button>
					</div>
				{:else if pairing === 'waiting'}
					{#if qrSvg}
						<div class="w-44 rounded-sm bg-white p-2">
							{@html qrSvg}
						</div>
					{/if}
					<span class="font-mono text-2xl tracking-widest">{status?.code}</span>
					<p class="text-surface-400 text-sm">{m.signal_pairing_waiting_label()}</p>
					<div class="flex items-center gap-2">
						<Button variant="ghost" size="sm" onclick={copyCode}>
							<Icon icon="tabler:copy" size={16} />
							{copyFeedback ? m.signal_pairing_copied() : m.signal_pairing_copy_code()}
						</Button>
						<Button variant="neutral" disabled={pairingBusy} onclick={stopPairing}>
							{m.signal_pairing_stop()}
						</Button>
					</div>
					<p class="text-surface-400 text-xs">{remainingLabel}</p>
				{:else}
					<p class="text-surface-400 text-sm">{m.signal_remote_access_no_devices()}</p>
					<Button variant="primary" loading={pairingBusy} onclick={startPairing}>
						{m.signal_pairing_start()}
					</Button>
				{/if}

				{#if pairingError}
					<p class="text-error-400 text-sm">{pairingError}</p>
				{/if}

				<div class="mt-2">
					<SignalArmedToggle />
				</div>
			</div>
		{:else}
			<SignalDevicePane onPairDevice={startPairing} />
		{/if}
	</div>
</div>
