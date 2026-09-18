import { goto } from '$app/navigation';
import { OpenFolder, SettingsModal } from '$components/modals';
import { baseStructuresData } from '$lib/data';
import { clearSessionPersistence, getStoredSessionId } from '$lib/utils/sessionPersistence';
import { send } from '$lib/utils/websocketUtils';
import { applySettings, getAppState, getModalState } from '$states';
import { MessageType } from '$types';
import * as m from '$i18n/messages';

export type NavActions = {
	save(): void;
	eject(): Promise<void>;
	openFolder(): Promise<void>;
	settings(): Promise<void>;
};

// A factory, not bare exports: these close over context reads that must happen
// during component initialization.
export function createNavActions(): NavActions {
	const appState = getAppState();
	const modal = getModalState();

	return {
		save() {
			appState.writeSave().catch((error: unknown) => {
				console.error('Error writing save:', error);
			});
		},

		async eject() {
			const sessionId = getStoredSessionId();
			if (sessionId) {
				send(MessageType.EJECT_SESSION, { session_id: sessionId });
			}
			appState.resetState();
			baseStructuresData.reset();
			clearSessionPersistence();
			await goto('/overview');
		},

		async openFolder() {
			// @ts-ignore
			await modal.showModal(OpenFolder, { title: m.open_folder() });
		},

		async settings() {
			// @ts-ignore
			const result = await modal.showModal<string>(SettingsModal, {
				title: m.settings(),
				settings: appState.settings
			});

			if (result) {
				applySettings();
				setTimeout(() => {
					location.reload();
				}, 500);
			}
		}
	};
}
