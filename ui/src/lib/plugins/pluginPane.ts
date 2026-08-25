export type PaneMode = 'run' | 'code';

export const PANE_MODES: readonly PaneMode[] = ['run', 'code'];

export const MODE_LABELS: Record<PaneMode, string> = {
	run: 'Run',
	code: 'Code'
};

const AVAILABILITY: Record<PaneMode, (plugin: { bundled: boolean }) => boolean> = {
	run: () => true,
	code: (plugin) => !plugin.bundled
};

export function availableModes(plugin: { bundled: boolean }): PaneMode[] {
	return PANE_MODES.filter((mode) => AVAILABILITY[mode](plugin));
}

export function isModeAvailable(mode: PaneMode, plugin: { bundled: boolean }): boolean {
	return AVAILABILITY[mode](plugin);
}

export function resolveMode(
	raw: string | null,
	plugin: { bundled: boolean } | undefined
): PaneMode {
	if (!plugin) return 'run';
	if (raw && PANE_MODES.includes(raw as PaneMode) && isModeAvailable(raw as PaneMode, plugin)) {
		return raw as PaneMode;
	}
	return 'run';
}

const PLUGIN_PATH = /^\/plugins\/([^/]+)\/?$/;

/**
 * The plugin a navigation target lands on, or `null` for anywhere else —
 * including `/plugins` itself, which leaves the detail pane behind.
 */
export function pluginIdFromPath(pathname: string | null | undefined): string | null {
	if (!pathname) return null;
	const match = PLUGIN_PATH.exec(pathname);
	if (!match) return null;
	try {
		return decodeURIComponent(match[1]);
	} catch {
		return match[1];
	}
}

/**
 * `nextId: null` means "not switching to any particular plugin" — closing the
 * editor pane in place rather than navigating to another plugin. It never
 * matches `editor.pluginId`, so the only way to stay safe is `!editor.dirty`.
 */
export function leaveIsSafe(
	editor: { pluginId: string | null; dirty: boolean },
	nextId: string | null
): boolean {
	return editor.pluginId === null || !editor.dirty || editor.pluginId === nextId;
}
