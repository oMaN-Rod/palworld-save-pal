import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
import { send } from '$lib/utils/websocketUtils';
import { MessageType } from '$types';

/** The Tauri webview drops `<a target="_blank">` navigations, so a desktop click
 *  is cancelled and the url handed to the host OS over the socket instead. */
export function openExternalLink(event: MouseEvent, url: string): void {
	if (PUBLIC_DESKTOP_MODE !== 'true') return;
	event.preventDefault();
	send(MessageType.OPEN_URL, url);
}
