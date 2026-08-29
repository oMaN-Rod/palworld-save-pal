import { MessageType } from '$types';
import { beforeEach, describe, expect, it, vi } from 'vitest';

let desktopMode = 'false';
const sent: Array<{ type: string; data: unknown }> = [];

vi.mock('$env/static/public', () => ({
	get PUBLIC_DESKTOP_MODE() {
		return desktopMode;
	}
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: string, data: unknown) => {
		sent.push({ type, data });
	}
}));

const { openExternalLink } = await import('./externalLink');

const clickEvent = () => {
	let defaultPrevented = false;
	return {
		preventDefault: () => {
			defaultPrevented = true;
		},
		get defaultPrevented() {
			return defaultPrevented;
		}
	} as unknown as MouseEvent;
};

beforeEach(() => {
	sent.length = 0;
	desktopMode = 'false';
});

describe('openExternalLink', () => {
	it('lets the browser follow the anchor on the web build', () => {
		const event = clickEvent();
		openExternalLink(event, 'https://github.com/oMaN-Rod/palworld-save-pal/releases');

		expect(event.defaultPrevented).toBe(false);
		expect(sent).toEqual([]);
	});

	it('hands the url to the host over the socket in desktop mode', () => {
		desktopMode = 'true';
		const event = clickEvent();
		openExternalLink(event, 'https://github.com/oMaN-Rod/palworld-save-pal/releases');

		expect(event.defaultPrevented).toBe(true);
		expect(sent).toEqual([
			{
				type: MessageType.OPEN_URL,
				data: 'https://github.com/oMaN-Rod/palworld-save-pal/releases'
			}
		]);
	});
});
