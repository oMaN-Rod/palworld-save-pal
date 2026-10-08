import type { WSMessageHandler } from '$lib/ws/types';
import { MessageType } from '$types';
import type { NexusDownloadReply, NexusLinkPush } from '$types';
import { getModsState, getNexusState } from '$states';
import { settled } from './modsHandler';

/** Any reply made on the account's behalf can report the sign-in gone, or still held despite a failure. */
function noteSession(data: { signed_in?: boolean; error?: { code?: string } } | undefined): void {
	const code = data?.error?.code;
	if (code === undefined) return;
	const state = getNexusState();
	if (code === 'sign_in_expired' || code === 'sign_in_required') state.signedOut();
	else if (data?.signed_in === true) state.signedIn = true;
}

export const nexusAccountGetHandler: WSMessageHandler = {
	type: MessageType.NEXUS_ACCOUNT_GET,
	async handle(data) {
		const state = getNexusState();
		state.finishBusy('account');
		noteSession(data);
		if (settled(MessageType.NEXUS_ACCOUNT_GET, data)) return;
		getModsState().clearLastError(MessageType.NEXUS_SIGN_IN);
		state.applyAccount(data);
	}
};

export const nexusSignInHandler: WSMessageHandler = {
	type: MessageType.NEXUS_SIGN_IN,
	async handle(data) {
		const state = getNexusState();
		state.finishBusy('signIn');
		if (data?.error?.code === 'sign_in_cancelled') return;
		noteSession(data);
		if (settled(MessageType.NEXUS_SIGN_IN, data)) return;
		getModsState().clearLastError(MessageType.NEXUS_ACCOUNT_GET);
		state.applyAccount(data);
	}
};

/** The cancelled sign-in answers for itself under `nexus_sign_in`. */
export const nexusSignInCancelHandler: WSMessageHandler = {
	type: MessageType.NEXUS_SIGN_IN_CANCEL,
	async handle(data) {
		settled(MessageType.NEXUS_SIGN_IN_CANCEL, data);
	}
};

export const nexusSignOutHandler: WSMessageHandler = {
	type: MessageType.NEXUS_SIGN_OUT,
	async handle(data) {
		const state = getNexusState();
		state.finishBusy('account');
		if (settled(MessageType.NEXUS_SIGN_OUT, data)) return;
		state.applyAccount(data);
	}
};

export const nexusCategoriesHandler: WSMessageHandler = {
	type: MessageType.NEXUS_CATEGORIES,
	async handle(data) {
		const state = getNexusState();
		state.finishBusy('categories');
		noteSession(data);
		if (settled(MessageType.NEXUS_CATEGORIES, data)) return;
		state.applyCategories(data);
	}
};

export const nexusSearchHandler: WSMessageHandler = {
	type: MessageType.NEXUS_SEARCH,
	async handle(data) {
		const state = getNexusState();
		state.finishBusy('searching');
		if (settled(MessageType.NEXUS_SEARCH, data)) return;
		state.applySearch(data);
	}
};

export const nexusModFilesHandler: WSMessageHandler = {
	type: MessageType.NEXUS_MOD_FILES,
	async handle(data) {
		const state = getNexusState();
		state.finishBusy('files');
		if (settled(MessageType.NEXUS_MOD_FILES, data)) return;
		state.applyFiles(data);
	}
};

export const nexusDownloadHandler: WSMessageHandler = {
	type: MessageType.NEXUS_DOWNLOAD,
	async handle(data: NexusDownloadReply) {
		const state = getNexusState();
		state.finishDownload(data.nexus_mod_id, data.file_id);
		noteSession(data);
		if (settled(MessageType.NEXUS_DOWNLOAD, data, data.target_id)) return;
		if (data.needs_decisions) {
			state.holdDecisions(data);
			return;
		}
		state.recordInstalled(data);
		getModsState().loadLibrary();
	}
};

export const nexusLinkSubscribeHandler: WSMessageHandler = {
	type: MessageType.NEXUS_LINK_SUBSCRIBE,
	async handle(data) {
		settled(MessageType.NEXUS_LINK_SUBSCRIBE, data);
	}
};

/** A push: it carries a link, an expired link, or a rejection — none of them a refusal. */
export const nexusLinkHandler: WSMessageHandler = {
	type: MessageType.NEXUS_LINK,
	async handle(data: NexusLinkPush) {
		getNexusState().pushLink(data);
	}
};

export const nexusHandlerStatusHandler: WSMessageHandler = {
	type: MessageType.NEXUS_HANDLER_STATUS,
	async handle(data) {
		const state = getNexusState();
		state.finishBusy('handler');
		if (settled(MessageType.NEXUS_HANDLER_STATUS, data)) return;
		state.applyHandler(data);
	}
};

export const nexusHandlerRegisterHandler: WSMessageHandler = {
	type: MessageType.NEXUS_HANDLER_REGISTER,
	async handle(data) {
		const state = getNexusState();
		state.finishBusy('handler');
		if (settled(MessageType.NEXUS_HANDLER_REGISTER, data)) return;
		state.applyHandler(data);
	}
};

export const modUpdateCheckHandler: WSMessageHandler = {
	type: MessageType.MOD_UPDATE_CHECK,
	async handle(data) {
		const state = getNexusState();
		state.finishBusy('checkingUpdates');
		if (settled(MessageType.MOD_UPDATE_CHECK, data, data?.target_id ?? undefined)) {
			state.forgetUpdateRun(data?.target_id ?? undefined);
			return;
		}
		state.applyUpdates(data);
	}
};

export const modUpdateIgnoreHandler: WSMessageHandler = {
	type: MessageType.MOD_UPDATE_IGNORE,
	async handle(data) {
		const state = getNexusState();
		state.finishBusy('ignoring');
		if (settled(MessageType.MOD_UPDATE_IGNORE, data)) return;
		state.applyIgnore(data);
	}
};

export const nexusHandlers: WSMessageHandler[] = [
	nexusAccountGetHandler,
	nexusSignInHandler,
	nexusSignInCancelHandler,
	nexusSignOutHandler,
	nexusCategoriesHandler,
	nexusSearchHandler,
	nexusModFilesHandler,
	nexusDownloadHandler,
	nexusLinkSubscribeHandler,
	nexusLinkHandler,
	nexusHandlerStatusHandler,
	nexusHandlerRegisterHandler,
	modUpdateCheckHandler,
	modUpdateIgnoreHandler
];
