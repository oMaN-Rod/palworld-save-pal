const LEVEL = 'level.sav';

export function baseName(path: string): string {
	return path.slice(path.lastIndexOf('/') + 1).toLowerCase();
}

function dirOf(path: string): string {
	const i = path.lastIndexOf('/');
	return i === -1 ? '' : path.slice(0, i + 1);
}

/**
 * Directory of the shallowest `Level.sav`, or null when there is none. A save
 * folder routinely contains whole copies of itself — the game's `backups/`,
 * PSP's own `.psp-backup/` — and each copy carries a `Level.sav` and a
 * `Players/` of its own, so a name match alone picks whichever the directory
 * listing happened to yield first.
 */
export function saveRoot(paths: string[]): string | null {
	let root: string | null = null;
	let depth = Infinity;
	for (const path of paths) {
		if (baseName(path) !== LEVEL) continue;
		const dir = dirOf(path);
		const d = dir === '' ? 0 : dir.split('/').length - 1;
		if (d < depth) {
			depth = d;
			root = dir;
		}
	}
	return root;
}

/** A file the save itself owns: its own top-level `.sav`s and `Players/*`. */
export function underSaveRoot(path: string, root: string): boolean {
	if (!path.startsWith(root)) return false;
	const rest = path.slice(root.length);
	return !rest.includes('/') || /^players\/[^/]+$/.test(rest.toLowerCase());
}
