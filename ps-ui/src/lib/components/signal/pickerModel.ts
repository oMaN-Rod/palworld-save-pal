import type { SaveModProfile } from '$types';

export interface LocalSaveEntry {
	path: string;
	name: string;
	save_type: string;
	modified_ms: number;
	world_key?: string;
	mod_profile?: SaveModProfile | null;
}

export interface GamepassSaveEntry {
	save_id: string;
	world_name: string;
	player_count: number;
	last_modified: number;
}

export interface BrowseEntry {
	name: string;
	path: string;
	is_dir: boolean;
}

export interface BrowseFrame {
	path: string;
	entries: BrowseEntry[];
}

export type PickerSection = 'steam' | 'gamepass' | 'server' | 'browse';

export function sortByRecency<T>(items: T[], getTimestamp: (item: T) => number): T[] {
	return [...items].sort((a, b) => {
		const aTs = getTimestamp(a);
		const bTs = getTimestamp(b);
		const aUnknown = aTs <= 0;
		const bUnknown = bTs <= 0;
		if (aUnknown && bUnknown) return 0;
		if (aUnknown) return 1;
		if (bUnknown) return -1;
		return bTs - aTs;
	});
}

export function sortLocalSaves(saves: LocalSaveEntry[]): LocalSaveEntry[] {
	return sortByRecency(saves, (save) => save.modified_ms);
}

export function sortGamepassSaves(saves: GamepassSaveEntry[]): GamepassSaveEntry[] {
	return sortByRecency(saves, (save) => save.last_modified);
}

export function hasLevelSav(entries: BrowseEntry[]): boolean {
	return levelSavPath(entries) !== null;
}

export function levelSavPath(entries: BrowseEntry[]): string | null {
	return entries.find((entry) => !entry.is_dir && entry.name === 'Level.sav')?.path ?? null;
}

export function pushFrame(stack: BrowseFrame[], frame: BrowseFrame): BrowseFrame[] {
	return [...stack, frame];
}

export function popFrame(stack: BrowseFrame[]): BrowseFrame[] {
	return stack.length > 1 ? stack.slice(0, -1) : stack;
}

export function popToIndex(stack: BrowseFrame[], index: number): BrowseFrame[] {
	if (index < 0 || index >= stack.length) return stack;
	return stack.slice(0, index + 1);
}

export function replyError(data: unknown): string | null {
	if (typeof data !== 'object' || data === null) return null;
	const error = (data as { error?: unknown }).error;
	return typeof error === 'string' ? error : null;
}

export function isServerRunning(server: { status?: { running: boolean } }): boolean {
	return server.status?.running ?? false;
}
