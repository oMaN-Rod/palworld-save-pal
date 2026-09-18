import type { ModUpdate } from '$types';

export type UpdateAction =
	| { kind: 'none' }
	| { kind: 'update'; version: string; fileId: number }
	| { kind: 'ignored'; version: string };

/** Nexus is desktop-only, so a remote session never offers an update. */
export function updateAction(
	update: ModUpdate | undefined,
	mode: { desktop: boolean; remote: boolean }
): UpdateAction {
	if (!update || !mode.desktop || mode.remote) return { kind: 'none' };
	if (update.state === 'ignored') {
		return { kind: 'ignored', version: update.ignored_version ?? update.latest?.version ?? '' };
	}
	if (update.state !== 'available' || !update.latest) return { kind: 'none' };
	return { kind: 'update', version: update.latest.version, fileId: update.latest.file_id };
}
