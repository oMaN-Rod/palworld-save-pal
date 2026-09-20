/**
 * PalStudio's own network policy (Settings → Network page): load/save the
 * redacted config over the REST API, surface tailscale/funnel/UPnP status,
 * and detect the PIN lock so the SPA can route to the unlock page.
 *
 * This is deliberately separate from the Palworld *game server* management
 * state — those are servers the tool edits; this is the tool itself.
 */

export type ListenMode = 'localhost' | 'lan' | 'tailscale' | 'wan';
export type AuthScope = 'never' | 'networkonly' | 'always';
/** What an EMPTY allowlist falls back to; listed addresses always apply. */
export type AllowMode = 'open' | 'balanced' | 'strict';
/** How remote peers may load the app and its assets; loopback is exempt. */
export type AssetTransport = 'https' | 'https-http' | 'loopback';

export interface NetworkConfigDto {
	tier?: string;
	listen: ListenMode;
	port: number;
	/** Absent when empty on older servers — normalized on load. */
	allow: { connect?: string[]; write?: string[]; mode?: AllowMode };
	auth: { scope: AuthScope; pin_set: boolean; session_ttl_secs: number };
	upnp_enabled: boolean;
	funnel_enabled: boolean;
	/** Absent on older servers — normalized on load. */
	https_enabled?: boolean;
	asset_transport?: AssetTransport;
}

export interface FunnelStatusDto {
	on: boolean;
	urls: string[];
	targets: string[];
	text: string | null;
}

export interface NetworkStatusDto {
	tailscale: { available: boolean; logged_in: boolean; ipv4: string[] };
	funnel: FunnelStatusDto | null;
	upnp: { compiled: boolean; available: boolean };
}

export interface RuntimeInfoDto {
	tier?: string;
	running_as_service: boolean;
	service_supported: boolean;
	service_manager: string | null;
	service_installed: boolean;
	desktop_available: boolean;
}

export interface SaveResult {
	config: NetworkConfigDto;
	restart_required: boolean;
	warnings: string[];
}

async function jsonOrThrow<T>(resp: Response): Promise<T> {
	if (!resp.ok) {
		let message = `${resp.status}`;
		try {
			const body = await resp.json();
			if (body?.error) message = body.error;
		} catch {
			/* non-JSON error body */
		}
		throw new Error(message);
	}
	return resp.json() as Promise<T>;
}

/**
 * A hung backend (suspended process, dead proxy) must surface as an error,
 * not an eternal spinner: every fetch here carries a timeout. Status saves
 * run tailscale CLI subprocesses server-side, so theirs is the longest.
 */
async function fetchJson<T>(url: string, init: RequestInit = {}, timeoutMs = 15_000): Promise<T> {
	const resp = await fetch(url, { ...init, signal: AbortSignal.timeout(timeoutMs) });
	return jsonOrThrow<T>(resp);
}

class NetworkState {
	config = $state<NetworkConfigDto | null>(null);
	status = $state<NetworkStatusDto | null>(null);
	runtime = $state<RuntimeInfoDto | null>(null);
	loading = $state(false);
	/** True while the (CLI-backed) status probe is running. */
	loadingStatus = $state(false);
	saving = $state(false);
	switchingMode = $state(false);
	/** Set after a standalone switch: the service is gone and this page is dying. */
	standaloneNote = $state<string | null>(null);
	error = $state<string | null>(null);
	/** Warnings returned by the last save (e.g. funnel without a PIN). */
	warnings = $state<string[]>([]);
	restartPending = $state(false);

	async load(): Promise<void> {
		this.loading = true;
		this.error = null;
		try {
			await Promise.all([this.loadConfig(), this.loadStatus()]);
		} catch (error) {
			this.error = error instanceof Error ? error.message : String(error);
		} finally {
			this.loading = false;
		}
	}

	/**
	 * Config + runtime only — fast (no CLI subprocesses), so the settings
	 * form can render from it immediately. Status is loaded separately via
	 * loadStatus(): applying its late result must never overwrite form
	 * fields the user may already be editing.
	 */
	async loadConfig(): Promise<void> {
		try {
			const config = await fetchJson<NetworkConfigDto>('/api/network/config');
			// Older servers omit empty allowlists entirely; the form reads
			// these unconditionally, so make them arrays once, here.
			this.config = {
				...config,
				allow: {
					connect: config.allow?.connect ?? [],
					write: config.allow?.write ?? [],
					mode: config.allow?.mode ?? 'balanced'
				}
			};
			// Service control is deliberately a separately authenticated local
			// control plane. A normal localhost browser must still be able to
			// render and edit network policy when no admin token/session exists;
			// in that posture the optional runtime controls remain unavailable.
			try {
				this.runtime = await fetchJson<RuntimeInfoDto>('/api/network/runtime');
			} catch {
				this.runtime = null;
			}
			this.error = null;
		} catch (error) {
			// The panel gates its form on this resolving: a failure must
			// become a visible error, never an infinite "loading".
			this.error = error instanceof Error ? error.message : String(error);
			throw error;
		}
	}

	/**
	 * Live tailscale/funnel/UPnP status. Shells out to the tailscale CLI,
	 * so it can take a moment — refreshable on its own. A failed refresh
	 * keeps the previous status; only the first failure shows as an error.
	 *
	 * `quiet` (background auto-refresh) skips the spinner state and never
	 * surfaces errors — the page keeps showing the last known status until a
	 * manual refresh reports why it cannot update.
	 */
	async loadStatus(quiet = false): Promise<void> {
		if (!quiet) this.loadingStatus = true;
		try {
			this.status = await fetchJson<NetworkStatusDto>('/api/network/status', {}, 30_000);
		} catch (error) {
			if (!this.status && !quiet) {
				this.error = error instanceof Error ? error.message : String(error);
			}
		} finally {
			if (!quiet) this.loadingStatus = false;
		}
	}

	/**
	 * Switch between standalone and background service. The server
	 * (un)registers the platform service and exits; `onGone` decides what
	 * the page does once the old instance is going away.
	 */
	async switchRuntime(mode: 'service' | 'standalone'): Promise<string | null> {
		this.switchingMode = true;
		this.error = null;
		try {
			// Registering/unregistering a platform service can take a while.
			const result = await fetchJson<{ ok: boolean; message: string }>(
				'/api/network/runtime',
				{
					method: 'POST',
					headers: { 'content-type': 'application/json' },
					body: JSON.stringify({ mode })
				},
				30_000
			);
			return result.message;
		} catch (error) {
			this.error = error instanceof Error ? error.message : String(error);
			return null;
		} finally {
			this.switchingMode = false;
		}
	}

	async save(update: {
		listen: ListenMode;
		port: number;
		connectRules: string;
		writeRules: string;
		allowMode: AllowMode;
		scope: AuthScope;
		newPin?: string;
		upnpEnabled: boolean;
		funnelEnabled: boolean;
		httpsEnabled: boolean;
		assetTransport: AssetTransport;
	}): Promise<SaveResult | null> {
		this.saving = true;
		this.error = null;
		this.warnings = [];
		try {
			const body = {
				listen: update.listen,
				port: update.port,
				allow: {
					connect: update.connectRules.split('\n').map((l) => l.trim()).filter(Boolean),
					write: update.writeRules.split('\n').map((l) => l.trim()).filter(Boolean),
					mode: update.allowMode
				},
				auth: { scope: update.scope, new_pin: update.newPin ?? null },
				upnp_enabled: update.upnpEnabled,
				funnel_enabled: update.funnelEnabled,
				https_enabled: update.httpsEnabled,
				asset_transport: update.assetTransport
			};
			const result = await fetchJson<SaveResult>(
				'/api/network/config',
				{
					method: 'PUT',
					headers: { 'content-type': 'application/json' },
					body: JSON.stringify(body)
				},
				// Saving runs the tailscale CLI (probe + toggle) server-side.
				30_000
			);
			this.config = result.config;
			this.warnings = result.warnings;
			this.restartPending = result.restart_required;
			return result;
		} catch (error) {
			this.error = error instanceof Error ? error.message : String(error);
			return null;
		} finally {
			this.saving = false;
		}
	}

	/**
	 * True when the server answered 401 — the SPA then redirects to the
	 * server-rendered unlock page, which sets the session cookie.
	 */
	async isLocked(): Promise<boolean> {
		try {
			const resp = await fetch('/api/network/config', {
				signal: AbortSignal.timeout(5_000)
			});
			return resp.status === 401;
		} catch {
			return false;
		}
	}
}

const networkState = new NetworkState();

export const getNetworkState = (): NetworkState => networkState;
