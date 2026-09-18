import { browser } from '$app/environment';
import { PUBLIC_DESKTOP_MODE } from '$env/static/public';

// True only in the browser-only build (worker transport + in-memory stub DB).
// The desktop app and the Docker/server deployment both use the WebSocket
// transport against a real backend, so DB-backed and native-only features work
// there and must stay enabled.
export const isWebBuild = import.meta.env.VITE_TRANSPORT === 'worker';

export type OsFamily = 'windows' | 'macos' | 'linux' | 'other';

// Order matters: some Windows agents carry a trailing "Linux" token.
export function osFamilyFrom(userAgent: string): OsFamily {
	if (/Windows/i.test(userAgent)) return 'windows';
	if (/Mac OS X|Macintosh/i.test(userAgent)) return 'macos';
	if (/Linux|X11/i.test(userAgent)) return 'linux';
	return 'other';
}

type TauriGlobal = Record<string, unknown>;

// `PUBLIC_DESKTOP_MODE` is true for `dev:desktop` too, which also serves the UI
// at localhost:5173 in an ordinary browser. Without the __TAURI__ check that tab
// would paint a title bar whose close button does nothing.
export function isTauri(): boolean {
	return browser && (window as Window & { __TAURI__?: TauriGlobal }).__TAURI__ !== undefined;
}

export function desktopChrome(): boolean {
	return PUBLIC_DESKTOP_MODE === 'true' && isTauri();
}

export function osFamily(): OsFamily {
	return browser ? osFamilyFrom(navigator.userAgent) : 'other';
}
