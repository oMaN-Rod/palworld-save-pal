import { MessageType } from '$types';
import { beforeEach, describe, expect, it, vi } from 'vitest';

let storedLocale: string | undefined;
let activeLocale = 'en';
const setLocaleCalls: string[] = [];
const sent: Array<{ type: string; data: Record<string, unknown> }> = [];
const appState = { settings: { language: 'en' } as Record<string, unknown> };

vi.mock('$i18n/runtime', async (importOriginal) => {
	const actual = await importOriginal<Record<string, unknown>>();
	return {
		...actual,
		extractLocaleFromCookie: () => storedLocale,
		getLocale: () => activeLocale,
		setLocale: (code: string) => {
			setLocaleCalls.push(code);
			activeLocale = code;
		}
	};
});

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: string, data: Record<string, unknown>) => {
		sent.push({ type, data });
	}
}));

vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => ({}),
	getToastState: () => ({ add: () => {} })
}));

const { settingsHandler } = await import('./appStateHandler');

const row = (language: string) => ({
	language,
	save_dir: '',
	clone_prefix: '©️',
	new_pal_prefix: '🆕',
	debug_mode: false,
	cheat_mode: false
});

beforeEach(() => {
	setLocaleCalls.length = 0;
	sent.length = 0;
	appState.settings = { language: 'en' };
	activeLocale = 'en';
	storedLocale = undefined;
});

describe('settingsHandler', () => {
	it('is registered for the settings message type', () => {
		expect(settingsHandler.type).toBe(MessageType.GET_SETTINGS);
	});

	// The web build's sqlite falls back to in-memory whenever the OPFS pool is
	// held elsewhere, so every boot reports the seeded default. Applying it would
	// reset a user who has already chosen a language.
	it('does not reset the chosen locale when the backend reports the seeded default', async () => {
		storedLocale = 'fr';
		activeLocale = 'fr';

		await settingsHandler.handle(row('en'), { goto: vi.fn() });

		expect(setLocaleCalls).toEqual([]);
		expect(appState.settings.language).toBe('fr');
	});

	it('corrects a backend row that disagrees with the stored locale', async () => {
		storedLocale = 'fr';
		activeLocale = 'fr';

		await settingsHandler.handle(row('en'), { goto: vi.fn() });

		expect(sent).toHaveLength(1);
		expect(sent[0].type).toBe(MessageType.UPDATE_SETTINGS);
		expect(sent[0].data.language).toBe('fr');
		expect(sent[0].data.clone_prefix).toBe('©️');
	});

	it('adopts the backend language when the browser stores no locale of its own', async () => {
		storedLocale = undefined;

		await settingsHandler.handle(row('de'), { goto: vi.fn() });

		expect(setLocaleCalls).toEqual(['de']);
		expect(appState.settings.language).toBe('de');
		expect(sent).toEqual([]);
	});

	it('keeps the rest of the settings row', async () => {
		storedLocale = 'fr';
		activeLocale = 'fr';

		await settingsHandler.handle({ ...row('en'), cheat_mode: true }, { goto: vi.fn() });

		expect(appState.settings.cheat_mode).toBe(true);
		expect(appState.settings.new_pal_prefix).toBe('🆕');
	});
});
