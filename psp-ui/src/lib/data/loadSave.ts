import { goto } from '$app/navigation';
import * as m from '$i18n/messages';
import { recordSession } from '$lib/fs';
import { pushProgressMessage, sendBytes } from '$lib/utils/websocketUtils';
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
	pushProgressMessage(m.upload_loading_save());
	// A copy, because the worker transport transfers the buffer it is given and
	// `recordSession` below still needs the original.
	sendBytes(MessageType.LOAD_ZIP_FILE, zip.slice());
	const res = await recordSession({
		zipBytes: zip,
		name,
		savedAt: Date.now(),
		handle: source?.handle,
		writable: source?.writable
	});
	if (res.quota) {
		getToastState().add(m.upload_too_large(), m.toast_heads_up(), 'warning');
	}
}
