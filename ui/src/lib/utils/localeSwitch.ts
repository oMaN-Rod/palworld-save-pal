export type LocaleSwitchDeps = {
	getLocale: () => string;
	setLocale: (code: string, opts: { reload: boolean }) => void;
	bump: () => void;
	persist: (code: string) => void;
};

/**
 * `setLocale` must run before `persist`: it only reloads when the locale
 * actually changes, so setting it first makes the backend's settings echo
 * (which calls `setLocale` with the default `reload: true`) a no-op.
 */
export function switchLocale(code: string, deps: LocaleSwitchDeps): boolean {
	if (code === deps.getLocale()) return false;
	deps.setLocale(code, { reload: false });
	deps.bump();
	deps.persist(code);
	return true;
}

export type SettingsApplyDeps = LocaleSwitchDeps & { persistAll: () => void };

/**
 * Applies the settings a modal just edited. The settings echo no longer applies
 * the backend's language, so the edit has to switch the locale itself —
 * `switchLocale` persists the whole object on its way, leaving `persistAll` for
 * the case where only the other fields changed.
 */
export function applyEditedSettings(language: string, deps: SettingsApplyDeps): void {
	if (!switchLocale(language, deps)) deps.persistAll();
}
