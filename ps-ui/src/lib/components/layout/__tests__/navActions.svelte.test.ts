import { MessageType } from '$types';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { send, goto, appState, modal, baseStructuresData, persistence } = vi.hoisted(() => ({
	send: vi.fn(),
	goto: vi.fn(),
	appState: { saveFile: { name: 'Level.sav' }, writeSave: vi.fn(), resetState: vi.fn() },
	modal: { showModal: vi.fn() },
	baseStructuresData: { reset: vi.fn() },
	persistence: { sessionId: 'sess-1' as string | undefined, clear: vi.fn() }
}));

vi.mock('$lib/utils/websocketUtils', () => ({ send }));
vi.mock('$app/navigation', () => ({ goto }));
vi.mock('$lib/data', () => ({ baseStructuresData }));
vi.mock('$lib/utils/sessionPersistence', () => ({
	getStoredSessionId: () => persistence.sessionId,
	clearSessionPersistence: persistence.clear
}));
vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => modal,
	applySettings: vi.fn()
}));
vi.mock('$components/modals', () => ({ OpenFolder: {}, SettingsModal: {} }));

import { createNavActions } from '../navActions.svelte';

describe('createNavActions', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		persistence.sessionId = 'sess-1';
		appState.writeSave.mockResolvedValue(undefined);
		modal.showModal.mockResolvedValue(undefined);
	});

	it('save writes the save file', () => {
		createNavActions().save();

		expect(appState.writeSave).toHaveBeenCalledOnce();
	});

	it('eject ends the server session, clears state and routes to the overview', async () => {
		await createNavActions().eject();

		expect(send).toHaveBeenCalledWith(MessageType.EJECT_SESSION, { session_id: 'sess-1' });
		expect(appState.resetState).toHaveBeenCalledOnce();
		expect(baseStructuresData.reset).toHaveBeenCalledOnce();
		expect(persistence.clear).toHaveBeenCalledOnce();
		expect(goto).toHaveBeenCalledWith('/overview');
	});

	it('eject still resets locally when there is no stored session', async () => {
		persistence.sessionId = undefined;

		await createNavActions().eject();

		expect(send).not.toHaveBeenCalled();
		expect(appState.resetState).toHaveBeenCalledOnce();
		expect(goto).toHaveBeenCalledWith('/overview');
	});

	it('openFolder opens the folder modal', async () => {
		await createNavActions().openFolder();

		expect(modal.showModal).toHaveBeenCalledOnce();
	});
});
