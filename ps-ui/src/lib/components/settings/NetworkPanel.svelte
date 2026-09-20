<script lang="ts">
	import { onMount } from 'svelte';
	import { Button, Card, Input } from '$components/ui';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { getNetworkState, type AssetTransport, type AuthScope, type ListenMode } from '$states';
	import { getModalState } from '$states';
	import { Switch } from '@skeletonlabs/skeleton-svelte';
	import type { CheckedChangeDetails } from '@zag-js/switch';
	import * as m from '$i18n/messages';

	const network = getNetworkState();

	// Native selects, not the type-to-search Combobox: a `<select>` can only
	// hold a value the user actually picked, so a save can never silently
	// revert the auth scope.
	const listenOptions: { value: ListenMode; label: string }[] = [
		{ value: 'localhost', label: m.network_listen_localhost() },
		{ value: 'lan', label: m.network_listen_lan() },
		{ value: 'tailscale', label: m.network_listen_tailscale() },
		{ value: 'wan', label: m.network_listen_wan() }
	];

	const authOptions: { value: AuthScope; label: string }[] = [
		{ value: 'never', label: m.network_auth_never() },
		{ value: 'networkonly', label: m.network_auth_network() },
		{ value: 'always', label: m.network_auth_always() }
	];

	const allowModeOptions: { value: 'open' | 'balanced' | 'strict'; label: string }[] = [
		{ value: 'open', label: m.network_allow_mode_open() },
		{ value: 'balanced', label: m.network_allow_mode_balanced() },
		{ value: 'strict', label: m.network_allow_mode_strict() }
	];

	let listen = $state<ListenMode>('localhost');
	let port = $state<number>(5174);
	let connectRules = $state('');
	let writeRules = $state('');
	let allowMode = $state<'open' | 'balanced' | 'strict'>('balanced');
	let authScope = $state<AuthScope>('never');
	let newPin = $state('');
	let confirmPin = $state('');
	let clearPin = $state(false);
	let upnpEnabled = $state(false);
	let funnelEnabled = $state(false);
	/** True after enabling Funnel auto-raised the PIN scope (server rule). */
	let funnelScopeForced = $state(false);
	let httpsEnabled = $state(false);
	let assetTransport = $state<AssetTransport>('https');
	/** True after enabling native HTTPS auto-turned Funnel off (server rule). */
	let httpsForcedFunnelOff = $state(false);
	let saved = $state(false);
	let switching = $state(false);
	let standaloneNote = $state<string | null>(null);

	// The form only renders once THIS mount fetched the config: the fields
	// never exist in a half-loaded state, so a slow fetch can no longer
	// clobber edits the user already made (the "scope/funnel revert on
	// reload" bug). Status is fetched separately and never touches fields.
	let ready = $state(false);

	const listenHint = $derived(
		{
			localhost: m.network_listen_localhost_hint(),
			lan: m.network_listen_lan_hint(),
			tailscale: m.network_listen_tailscale_hint(),
			wan: m.network_listen_wan_hint()
		}[listen]
	);

	const assetTransportOptions: { value: AssetTransport; label: string }[] = [
		{ value: 'https', label: m.network_transport_https() },
		{ value: 'https-http', label: m.network_transport_https_http() },
		{ value: 'loopback', label: m.network_transport_loopback() }
	];

	onMount(() => {
		network
			.loadConfig()
			.then(() => {
				const config = network.config;
				if (!config) return;
				listen = config.listen;
				port = config.port;
				connectRules = (config.allow.connect ?? []).join('\n');
				writeRules = (config.allow.write ?? []).join('\n');
				allowMode = config.allow.mode ?? 'balanced';
				authScope = config.auth.scope;
				upnpEnabled = config.upnp_enabled;
				funnelEnabled = config.funnel_enabled;
				httpsEnabled = config.https_enabled ?? false;
				assetTransport = config.asset_transport ?? 'https';
				funnelScopeForced = false;
				httpsForcedFunnelOff = false;
				ready = true;
			})
			.catch((error) => {
				// Fetch failures already set network.error inside loadConfig;
				// a throw from the block above means the payload shape
				// surprised us — surface it instead of spinning forever.
				if (!network.error) {
					network.error = error instanceof Error ? error.message : String(error);
				}
			});
		// Live status (tailscale CLI): slow, refreshable, form-independent.
		network.loadStatus();
		// ...and it STAYS live: a quiet background refresh keeps the status
		// card (funnel/UPnP/tailscale posture) current while the page is
		// open. The OPTIONS above are deliberately not refreshed — edits in
		// progress must never be clobbered by a re-read.
		const statusTimer = setInterval(() => void network.loadStatus(true), 15_000);
		return () => clearInterval(statusTimer);
	});

	const pinsMatch = $derived(newPin === confirmPin);
	// Tier gating: a hand-launched local webapp only gets the port editor
	// (plus the runtime switch); the Tauri desktop never reaches this page.
	const isLocalWebapp = $derived(
		network.config?.tier === 'localwebapp' || network.runtime?.tier === 'localwebapp'
	);

	// Mirrors the server's security_warnings, computed live so the banner
	// reacts to edits before they are saved.
	const pinEffective = $derived(
		clearPin ? false : Boolean(newPin) || (network.config?.auth.pin_set ?? false)
	);
	const postureWarnings = $derived.by(() => {
		const warnings: string[] = [];
		if (listen !== 'localhost' && authScope === 'never' && !pinEffective) {
			warnings.push(m.network_warning_no_pin());
		}
		if (authScope !== 'never' && !pinEffective) {
			warnings.push(m.network_warning_pin_missing());
		}
		// Mirror of the server's asset-transport posture warnings, live so a
		// pending relaxation is visible before it is saved.
		if (listen !== 'localhost') {
			if (assetTransport === 'https-http') {
				warnings.push(m.network_warning_cleartext());
			} else if (assetTransport === 'loopback') {
				warnings.push(m.network_warning_loopback_only());
			} else if (!httpsEnabled && !funnelEnabled) {
				warnings.push(m.network_warning_no_https());
			}
		}
		return warnings;
	});

	// The toggle is the saved intent; this is what tailscale reports right
	// now. When they disagree, saving reconciles reality with the toggle.
	const funnelLive = $derived(network.status?.funnel);
	const funnelMismatch = $derived(
		network.status?.tailscale.available === true &&
			funnelLive != null &&
			funnelLive.on !== funnelEnabled
	);
	// The server rejects Funnel unless the PIN applies to every session; the
	// notice covers both the auto-raise and a manual scope drop while on.
	const funnelNeedsAlwaysScope = $derived(funnelEnabled && authScope !== 'always');

	async function switchToService() {
		const confirmed = await getModalState().showConfirmModal({
			message: m.network_runtime_switch_confirm_service()
		});
		if (!confirmed) return;
		const message = await network.switchRuntime('service');
		if (message === null) return;
		switching = true;
		// The old instance exits; poll until the service has the port again.
		const started = Date.now();
		const poll = setInterval(async () => {
			try {
				const resp = await fetch('/api/network/config');
				if (resp.ok) {
					clearInterval(poll);
					location.reload();
				}
			} catch {
				/* still down */
			}
			if (Date.now() - started > 60_000) clearInterval(poll);
		}, 1500);
	}

	async function switchToStandalone() {
		const confirmed = await getModalState().showConfirmModal({
			message: m.network_runtime_switch_confirm_standalone()
		});
		if (!confirmed) return;
		const message = await network.switchRuntime('standalone');
		if (message === null) return;
		standaloneNote = m.network_runtime_standalone_note();
	}

	async function save() {
		saved = false;
		if ((newPin || confirmPin) && !pinsMatch) return;
		const result = await network.save({
			listen,
			port,
			connectRules,
			writeRules,
			allowMode,
			scope: funnelEnabled && authScope !== 'always' ? 'always' : authScope,
			// Empty string clears the PIN server-side; undefined keeps it.
			newPin: clearPin ? '' : newPin ? newPin : undefined,
			upnpEnabled,
			funnelEnabled,
			httpsEnabled,
			assetTransport
		});
		if (result) {
			saved = true;
			newPin = '';
			confirmPin = '';
			clearPin = false;
			funnelScopeForced = false;
			httpsForcedFunnelOff = false;
			// If a side effect failed (e.g. the tailscale CLI errored), the
			// server rolled that toggle back — reflect the truth in the form.
			funnelEnabled = result.config.funnel_enabled;
			upnpEnabled = result.config.upnp_enabled;
			httpsEnabled = result.config.https_enabled ?? false;
			assetTransport = result.config.asset_transport ?? 'https';
			authScope = result.config.auth.scope;
			allowMode = result.config.allow?.mode ?? 'balanced';
			// Re-read what tailscale says now that the save applied.
			network.loadStatus();
		}
	}
</script>

<div class="flex w-full flex-col gap-3">
	{#if network.error}
		<Card class="border-error-500/50">
			<p class="text-error-400">{m.network_load_failed()}: {network.error}</p>
		</Card>
	{/if}

	{#if !ready && !network.error}
		<Card><p class="animate-pulse">{m.loading()}</p></Card>
	{:else if network.config && ready}
		{#if postureWarnings.length}
			<Card class="border-warning-500/60">
				<ul class="flex flex-col gap-1">
					{#each postureWarnings as warning (warning)}
						<li class="text-sm text-warning-500">⚠ {warning}</li>
					{/each}
				</ul>
			</Card>
		{/if}

		<!-- Exposure: listen mode + port -->
		<Card class="flex flex-col gap-2 p-3">
			<h2 class="h4 flex items-center gap-2">
				<Icon icon="tabler:world" size={16} /> {m.network_listen_mode()}
			</h2>
			<div class="grid grid-cols-[1fr_7rem] items-end gap-2">
				<label class="flex flex-col gap-1">
					<span class="text-sm">{m.network_listen_mode()}</span>
					<select
						class="bg-surface-900 border-surface-700 rounded-sm border px-2 py-1.5 text-sm"
						bind:value={listen}
					>
						{#each listenOptions as option (option.value)}
							<option value={option.value}>{option.label}</option>
						{/each}
					</select>
				</label>
				<label class="flex flex-col gap-1">
					<span class="text-sm">{m.network_port()}</span>
					<Input type="number" min={1} max={65535} bind:value={port} />
				</label>
			</div>
			<p class="text-xs text-surface-400">{listenHint}</p>
			{#if isLocalWebapp}
				<p class="text-xs text-surface-400">{m.network_local_mode_note()}</p>
			{/if}
		</Card>

		{#if !isLocalWebapp}
			<!-- Transport: asset streaming policy + native HTTPS -->
			<Card class="flex flex-col gap-2 p-3">
				<h2 class="h4 flex items-center gap-2">
					<Icon icon="tabler:shield-lock" size={16} /> {m.network_transport()}
				</h2>
				<label class="flex flex-col gap-1">
					<span class="text-sm">{m.network_transport()}</span>
					<select
						class="bg-surface-900 border-surface-700 rounded-sm border px-2 py-1.5 text-sm"
						bind:value={assetTransport}
					>
						{#each assetTransportOptions as option (option.value)}
							<option value={option.value}>{option.label}</option>
						{/each}
					</select>
					<span class="text-xs text-surface-400">{m.network_transport_hint()}</span>
				</label>
				<div class="flex flex-col gap-1.5 border-t border-surface-800 pt-2">
					<div class="flex items-center gap-2">
						<Switch
							name="https"
							checked={httpsEnabled}
							onCheckedChange={(e: CheckedChangeDetails) => {
								httpsEnabled = e.checked;
								// The server rejects both on: Funnel forwards to
								// this port over plain HTTP, which a TLS
								// listener declines. Resolve it here so the
								// save cannot 400.
								if (httpsEnabled && funnelEnabled) {
									funnelEnabled = false;
									httpsForcedFunnelOff = true;
								}
								if (!httpsEnabled) httpsForcedFunnelOff = false;
							}}
						/>
						<span class="text-sm">{m.network_https_host()}</span>
					</div>
					<p class="text-xs text-surface-400">{m.network_https_host_hint()}</p>
					{#if httpsForcedFunnelOff}
						<p class="text-xs text-warning-500">{m.network_https_funnel_note()}</p>
					{/if}
				</div>
			</Card>

			<!-- PIN protection -->
			<Card class="flex flex-col gap-2 p-3">
				<h2 class="h4 flex items-center gap-2">
					<Icon icon="tabler:lock" size={16} /> {m.network_auth_scope()}
				</h2>
				<div class="grid grid-cols-[1fr_auto] items-center gap-2">
					<select
						class="bg-surface-900 border-surface-700 rounded-sm border px-2 py-1.5 text-sm"
						bind:value={authScope}
					>
						{#each authOptions as option (option.value)}
							<option value={option.value}>{option.label}</option>
						{/each}
					</select>
					<span class="text-xs {network.config.auth.pin_set ? 'text-success-500' : 'text-surface-400'}">
						{network.config.auth.pin_set ? m.network_pin_current_set() : m.network_pin_current_none()}
					</span>
				</div>
				<p class="text-xs text-surface-400">{m.network_auth_network_hint()}</p>
				<div class="grid grid-cols-2 gap-2">
					<Input type="password" bind:value={newPin} label={m.network_pin_new()} />
					<Input type="password" bind:value={confirmPin} label={m.network_pin_confirm()} />
				</div>
				{#if (newPin || confirmPin) && !pinsMatch}
					<p class="text-sm text-error-400">{m.network_pin_mismatch()}</p>
				{/if}
				{#if network.config.auth.pin_set}
					<label class="flex items-center gap-2 text-sm">
						<input type="checkbox" bind:checked={clearPin} />
						{m.network_pin_clear()}
					</label>
				{/if}
			</Card>

			<!-- Remote access: funnel + UPnP with live status -->
			<Card class="flex flex-col gap-2 p-3">
				<div class="flex items-center justify-between">
					<h2 class="h4 flex items-center gap-2">
						<Icon icon="tabler:topology-star" size={16} /> {m.network_tailscale()}
					</h2>
					<Button
						variant="ghost"
						size="icon"
						onclick={() => network.loadStatus()}
						disabled={network.loadingStatus}
						title={m.network_status()}
					>
						<Icon
							icon="tabler:refresh"
							class={network.loadingStatus ? 'animate-spin' : ''}
						/>
					</Button>
				</div>

				{#if network.status?.tailscale.available}
					<p class="text-xs text-surface-400">
						{m.network_tailscale_ips()}:
						<span class="font-mono">{network.status.tailscale.ipv4.join(', ') || '—'}</span>
					</p>

					<div class="flex items-center gap-2">
						<Switch
							name="funnel"
							checked={funnelEnabled}
						onCheckedChange={(e: CheckedChangeDetails) => {
							funnelEnabled = e.checked;
							// The server rejects Funnel unless the PIN
							// covers every session; adopt that scope with
							// the toggle so the save cannot 400.
							if (funnelEnabled && authScope !== 'always') {
								authScope = 'always';
								funnelScopeForced = true;
							}
							if (funnelEnabled && httpsEnabled) {
								httpsEnabled = false;
								httpsForcedFunnelOff = true;
							}
							if (!funnelEnabled) funnelScopeForced = false;
						}}
						/>
						<span class="text-sm">{m.network_funnel()}</span>
						{#if funnelLive}
							<span class="ml-auto text-xs {funnelLive.on ? 'text-success-500' : 'text-surface-400'}">
								{#if network.loadingStatus}
									…
								{:else if funnelLive.on}
									● {funnelLive.urls[0] ?? 'on'}
								{:else}
									○ {m.network_auth_never()}
								{/if}
							</span>
						{/if}
					</div>
					<p class="text-xs text-warning-500">{m.network_funnel_warning()}</p>
				{#if funnelScopeForced || funnelNeedsAlwaysScope}
					<p class="text-xs text-warning-500">{m.network_funnel_scope_forced()}</p>
				{/if}
					{#if funnelMismatch}
						<p class="text-xs text-surface-300">
							⚠ {funnelLive?.on ? m.network_funnel_live_off() : m.network_funnel_live_on()}
						</p>
					{/if}
					{#if funnelLive?.targets.length}
						<p class="text-xs text-surface-400">
							→ <span class="font-mono">{funnelLive.targets.join(', ')}</span>
						</p>
					{/if}
					{#if funnelLive?.text}
						<details class="text-xs">
							<summary class="cursor-pointer text-surface-300">{m.network_status()}</summary>
							<pre class="bg-surface-100/5 mt-1 max-h-32 overflow-auto rounded p-2">{funnelLive.text}</pre>
						</details>
					{/if}
				{:else if network.loadingStatus}
					<p class="animate-pulse text-xs text-surface-400">{m.loading()}</p>
				{:else}
					<p class="text-xs text-surface-400">{m.network_tailscale_not_found()}</p>
				{/if}

				<div class="flex flex-col gap-1.5 border-t border-surface-800 pt-2">
					<div class="flex items-center gap-2">
						<Switch
							name="upnp"
							checked={upnpEnabled}
							disabled={!network.status?.upnp.compiled}
							onCheckedChange={(e: CheckedChangeDetails) => (upnpEnabled = e.checked)}
						/>
						<span class="text-sm">
							{m.network_upnp()}
							{#if !network.status?.upnp.compiled}
								<span class="text-xs text-surface-400">({m.network_upnp_unsupported()})</span>
							{/if}
						</span>
					</div>
					<p class="text-xs text-warning-500">{m.network_upnp_warning()}</p>
				</div>
			</Card>

			<!-- Advanced: allowlists + runtime mode, collapsed by default -->
			<details class="text-sm">
				<summary class="cursor-pointer text-surface-300">IP / CIDR</summary>
			<Card class="mt-2 flex flex-col gap-2 p-3">
				<label class="flex flex-col gap-1">
					<span class="text-sm">{m.network_allow_mode()}</span>
					<select
						class="bg-surface-900 border-surface-700 rounded-sm border px-2 py-1.5 text-sm"
						bind:value={allowMode}
					>
						{#each allowModeOptions as option (option.value)}
							<option value={option.value}>{option.label}</option>
						{/each}
					</select>
					<span class="text-xs text-surface-400">{m.network_allow_mode_hint()}</span>
				</label>
				<label class="flex flex-col gap-1">
					<span class="text-sm">{m.network_allow_connect()}</span>
					<textarea
						class="textarea h-20 font-mono text-sm"
						placeholder="192.168.1.0/24&#10;100.71.3.9"
						bind:value={connectRules}
					></textarea>
					<span class="text-xs text-surface-400">{m.network_allow_connect_hint()}</span>
				</label>
					<label class="flex flex-col gap-1">
						<span class="text-sm">{m.network_allow_write()}</span>
						<textarea
							class="textarea h-20 font-mono text-sm"
							placeholder="192.168.1.7&#10;100.71.3.9"
							bind:value={writeRules}
						></textarea>
						<span class="text-xs text-surface-400">{m.network_allow_write_hint()}</span>
					</label>
				</Card>
			</details>

			{#if network.runtime}
				<details class="text-sm">
					<summary class="cursor-pointer text-surface-300">
						<Icon icon="tabler:cpu" size={14} class="mr-1 inline" />
						{m.network_runtime_mode()}
					</summary>
					<Card class="mt-2 flex flex-col gap-2 p-3">
						<p class="text-sm">
							{#if network.runtime.running_as_service}
								{m.network_runtime_current_service()}
							{:else}
								{m.network_runtime_current_standalone()}
							{/if}
							{#if network.runtime.service_installed && !network.runtime.running_as_service}
								{m.network_runtime_service_installed()}
							{/if}
						</p>
						{#if standaloneNote}
							<p class="text-sm text-warning-500">{standaloneNote}</p>
						{:else if switching}
							<p class="text-sm">{m.network_runtime_switching()}</p>
						{:else if network.runtime.service_supported}
							<div class="flex gap-2">
								{#if !network.runtime.service_installed}
									<Button onclick={switchToService} loading={network.switchingMode}>
										{m.network_runtime_switch_to_service()}
									</Button>
								{:else}
									<Button onclick={switchToStandalone} loading={network.switchingMode}>
										{m.network_runtime_switch_to_standalone()}
									</Button>
								{/if}
							</div>
						{:else}
							<p class="text-xs text-surface-400">{m.network_runtime_unsupported()}</p>
						{/if}
					</Card>
				</details>
			{/if}
		{/if}

		<!-- Save -->
		<div class="flex items-center gap-3">
			<Button onclick={save} loading={network.saving} disabled={Boolean((newPin || confirmPin) && !pinsMatch)}>
				{m.network_save()}
			</Button>
			{#if saved}
				<span class="text-sm text-success-500">
					{m.network_saved()}
					{#if network.restartPending}
						— {m.network_restart_pending()}
					{/if}
				</span>
			{/if}
		</div>
		{#each network.warnings as warning (warning)}
			<p class="text-sm text-warning-500">⚠ {warning}</p>
		{/each}
	{/if}
</div>
