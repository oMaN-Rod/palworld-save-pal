import { goto } from '$app/navigation';
import { send, pushProgressMessage } from '$lib/utils/websocketUtils';
import { recordSession } from '$lib/fs';
import { getAppState, getToastState } from '$states';
import { MessageType } from '$types';

export async function startSaveLoad(
	zip: Uint8Array,
	name: string,
	source?: { handle?: FileSystemDirectoryHandle; writable?: boolean }
): Promise<void> {
	const appState = getAppState();
	await goto('/loading');
	appState.resetState();
	pushProgressMessage('Loading save...');
	send(MessageType.LOAD_ZIP_FILE, Array.from(zip));
	const res = await recordSession({
		zipBytes: zip,
		name,
		savedAt: Date.now(),
		handle: source?.handle,
		writable: source?.writable
	});
	if (res.quota) {
		getToastState().add(
			'This save is too large to keep across reloads in this browser.',
			'Heads up',
			'warning'
		);
	}
}
