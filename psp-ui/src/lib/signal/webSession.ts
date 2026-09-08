import { browser } from '$app/environment';
import { getLiveActors } from '$lib/data/liveActors.svelte';
import { deviceLabel, saveStoredDesktop } from './credentialStore';
import { createSignalSession, type SignalSession } from './session.svelte';
import { createTurnProvider } from './turnCredential';

export { loadStoredDesktop as getStoredDesktop } from './credentialStore';

let instance: SignalSession | null = null;

function isDeviceCredentialData(
	data: unknown
): data is { deviceId: string; deviceSecret: string; meetRoom: string; desktopName: string } {
	if (!data || typeof data !== 'object') return false;
	const d = data as Record<string, unknown>;
	return (
		typeof d.deviceId === 'string' &&
		typeof d.deviceSecret === 'string' &&
		typeof d.meetRoom === 'string' &&
		typeof d.desktopName === 'string'
	);
}

export function getWebSignalSession(): SignalSession {
	if (!instance) {
		instance = createSignalSession({
			turnProvider: createTurnProvider(),
			forceRelay:
				import.meta.env.DEV && browser && new URLSearchParams(location.search).has('relay')
		});
		instance.onFrame((frame) => getLiveActors().applyFrame(frame));
		instance.message.subscribe((msg) => {
			if (msg.type !== 'device_credential' || !isDeviceCredentialData(msg.data)) return;
			saveStoredDesktop({ ...msg.data, pairedAtMs: Date.now() });
			try {
				instance!.send('device_credential', { name: deviceLabel() });
			} catch {
			}
		});
	}
	return instance;
}
