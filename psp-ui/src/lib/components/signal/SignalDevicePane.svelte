<script lang="ts">
	import { Button, Input } from '$components/ui';
	import * as m from '$i18n/messages';
	import { getModalState, getSignalState } from '$states';
	import type { SignalDeviceJson } from '$types';
	import SignalArmedToggle from './SignalArmedToggle.svelte';

	let { onPairDevice }: { onPairDevice?: () => void } = $props();

	const signalState = getSignalState();
	const modal = getModalState();

	let renamingId = $state<string | null>(null);
	let renameDraft = $state('');
	let deviceError = $state<string | null>(null);
	let revokingId = $state<string | null>(null);
	let resetBusy = $state(false);
	let resetError = $state<string | null>(null);
	let now = $state(Date.now());

	$effect(() => {
		const interval = setInterval(() => (now = Date.now()), 30000);
		return () => clearInterval(interval);
	});

	const connectedCount = $derived(signalState.devices.filter((device) => device.connected).length);

	function startRename(device: SignalDeviceJson) {
		deviceError = null;
		renamingId = device.deviceId;
		renameDraft = device.name;
	}

	function cancelRename(device: SignalDeviceJson) {
		if (renamingId === device.deviceId) renamingId = null;
	}

	async function commitRename(device: SignalDeviceJson) {
		if (renamingId !== device.deviceId) return;
		renamingId = null;
		const name = renameDraft.trim();
		if (!name || name === device.name) return;
		deviceError = null;
		try {
			await signalState.renameDevice(device.deviceId, name);
		} catch (error) {
			deviceError = error instanceof Error ? error.message : String(error);
		}
	}

	function handleRenameKeydown(event: KeyboardEvent, device: SignalDeviceJson) {
		if (event.key === 'Enter') {
			(event.currentTarget as HTMLInputElement).blur();
		} else if (event.key === 'Escape') {
			renameDraft = device.name;
			renamingId = null;
		}
	}

	async function revokeDevice(device: SignalDeviceJson) {
		const confirmed = await modal.showConfirmModal({
			title: m.signal_remote_access_revoke_confirm_title(),
			message: m.signal_remote_access_revoke_confirm_message({ name: device.name }),
			confirmText: m.signal_remote_access_revoke(),
			cancelText: m.cancel()
		});
		if (!confirmed) return;
		revokingId = device.deviceId;
		deviceError = null;
		try {
			await signalState.revokeDevice(device.deviceId);
		} catch (error) {
			deviceError = error instanceof Error ? error.message : String(error);
		} finally {
			revokingId = null;
		}
	}

	async function resetRemoteAccess() {
		const confirmed = await modal.showConfirmModal({
			title: m.signal_remote_access_reset_confirm_title(),
			message: m.signal_remote_access_reset_confirm_message(),
			confirmText: m.signal_remote_access_reset_confirm_button(),
			cancelText: m.cancel()
		});
		if (!confirmed) return;
		resetBusy = true;
		resetError = null;
		try {
			await signalState.resetRemoteAccess();
		} catch (error) {
			resetError = error instanceof Error ? error.message : String(error);
		} finally {
			resetBusy = false;
		}
	}

	function lastSeenLabel(device: SignalDeviceJson): string {
		void now;
		if (device.lastSeenMs === undefined) return m.signal_remote_access_last_seen_never();
		const minutes = Math.floor(Math.max(0, now - device.lastSeenMs) / 60000);
		if (minutes < 1) return m.signal_remote_access_last_seen_just_now();
		if (minutes < 60) return m.signal_remote_access_last_seen_minutes({ count: minutes });
		const hours = Math.floor(minutes / 60);
		if (hours < 24) return m.signal_remote_access_last_seen_hours({ count: hours });
		return m.signal_remote_access_last_seen_days({ count: Math.floor(hours / 24) });
	}

	const pairedOn = new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'short' });
</script>

<div class="flex min-h-0 flex-col gap-3">
	<div class="flex flex-wrap items-center gap-x-4 gap-y-2">
		<h2 class="text-lg font-semibold">{m.signal_remote_access_devices_title()}</h2>
		<span class="text-surface-400 text-sm">
			{m.signal_devices_summary({
				count: signalState.devices.length,
				connected: connectedCount
			})}
		</span>
		<div class="ml-auto flex items-center gap-2">
			{#if onPairDevice}
				<Button variant="primary" size="sm" onclick={onPairDevice}>{m.signal_pair_device()}</Button>
			{/if}
			<Button variant="danger" size="sm" loading={resetBusy} onclick={resetRemoteAccess}>
				{m.signal_remote_access_reset_button()}
			</Button>
		</div>
	</div>

	<SignalArmedToggle />

	{#if resetError}
		<p class="text-error-400 text-sm">{resetError}</p>
	{/if}
	{#if deviceError}
		<p class="text-error-400 text-sm">{deviceError}</p>
	{/if}

	<div class="border-surface-800 min-h-0 overflow-auto rounded-sm border">
		<table id="signal-device-table" class="w-full border-collapse text-sm">
			<thead>
				<tr class="border-surface-800 border-b">
					<th class="text-surface-400 px-3 py-2 text-left text-xs font-semibold uppercase">
						{m.signal_device_column()}
					</th>
					<th class="text-surface-400 px-3 py-2 text-left text-xs font-semibold uppercase">
						{m.signal_status_column()}
					</th>
					<th class="text-surface-400 px-3 py-2 text-left text-xs font-semibold uppercase">
						{m.signal_last_seen_column()}
					</th>
					<th class="text-surface-400 px-3 py-2 text-left text-xs font-semibold uppercase">
						{m.signal_paired_on()}
					</th>
					<th></th>
				</tr>
			</thead>
			<tbody>
				{#each signalState.devices as device (device.deviceId)}
					<tr class="border-surface-800 border-b last:border-b-0">
						<td class="px-3 py-2">
							{#if renamingId === device.deviceId}
								<Input
									inputClass="my-0 p-1 text-sm"
									bind:value={renameDraft}
									autocomplete="off"
									onValueChange={() => commitRename(device)}
									onblur={() => cancelRename(device)}
									onkeydown={(event: KeyboardEvent) => handleRenameKeydown(event, device)}
								/>
							{:else}
								<button
									type="button"
									class="hover:text-primary-400 truncate text-left font-medium"
									title={m.signal_remote_access_rename_aria({ name: device.name })}
									onclick={() => startRename(device)}
								>
									{device.name}
								</button>
							{/if}
						</td>
						<td class="px-3 py-2">
							{#if device.connected}
								<span
									class="bg-success-500/15 text-success-400 rounded-sm px-1.5 py-0.5 text-xs font-medium"
								>
									{m.signal_remote_access_connected_badge()}
								</span>
							{:else}
								<span class="text-surface-400 text-xs">{m.signal_device_idle()}</span>
							{/if}
						</td>
						<td class="text-surface-400 px-3 py-2 tabular-nums">{lastSeenLabel(device)}</td>
						<td class="text-surface-400 px-3 py-2 tabular-nums">
							{pairedOn.format(new Date(device.createdAtMs))}
						</td>
						<td class="px-3 py-2 text-right">
							<Button
								variant="ghost"
								size="sm"
								loading={revokingId === device.deviceId}
								onclick={() => revokeDevice(device)}
							>
								{m.signal_remote_access_revoke()}
							</Button>
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
</div>
