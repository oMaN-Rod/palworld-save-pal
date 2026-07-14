// Plain sessionStorage helpers for the session-id/selected-player reattach flow.
// Deliberately no library (svelte-persisted-state etc.) — two small string values.

const SESSION_ID_KEY = 'psp:session_id';
const SELECTED_PLAYER_UID_KEY = 'psp:selected_player_uid';

export function getStoredSessionId(): string | null {
	return sessionStorage.getItem(SESSION_ID_KEY);
}

export function setStoredSessionId(sessionId: string): void {
	sessionStorage.setItem(SESSION_ID_KEY, sessionId);
}

export function getStoredSelectedPlayerUid(): string | null {
	return sessionStorage.getItem(SELECTED_PLAYER_UID_KEY);
}

export function setStoredSelectedPlayerUid(uid: string): void {
	sessionStorage.setItem(SELECTED_PLAYER_UID_KEY, uid);
}

export function clearStoredSelectedPlayerUid(): void {
	sessionStorage.removeItem(SELECTED_PLAYER_UID_KEY);
}

export function clearSessionPersistence(): void {
	sessionStorage.removeItem(SESSION_ID_KEY);
	sessionStorage.removeItem(SELECTED_PLAYER_UID_KEY);
}

// True while a reattach_session request is awaiting its overview reply, so the
// loaded_save_files handler knows to re-select the stored player. Guards against
// a plain fresh load re-selecting a stale uid left over from a previous save.
let reattachPending = false;

export function markReattachPending(): void {
	reattachPending = true;
}

export function consumeReattachPending(): boolean {
	const was = reattachPending;
	reattachPending = false;
	return was;
}
