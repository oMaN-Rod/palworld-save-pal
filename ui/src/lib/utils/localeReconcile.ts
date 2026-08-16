export type LocaleReconcileDeps = {
	storedLocale: () => string | undefined;
	getLocale: () => string;
	setLocale: (code: string, opts: { reload: boolean }) => void;
	bump: () => void;
	persist: (code: string) => void;
};

/**
 * The settings row mirrors the locale the browser already stores; it does not
 * own it. A web session whose sqlite fell back to in-memory reports the seeded
 * default on every boot, so letting the row win would reset anyone who had
 * chosen a language. It applies only when the browser stores no locale of its
 * own, and is corrected when the two disagree.
 *
 * Returns the language the app should record in its settings.
 */
export function reconcileSettingsLocale(backendLocale: string, deps: LocaleReconcileDeps): string {
	const stored = deps.storedLocale();
	if (stored === undefined) {
		if (backendLocale !== deps.getLocale()) {
			deps.setLocale(backendLocale, { reload: false });
			deps.bump();
		}
		return backendLocale;
	}
	if (backendLocale !== stored) deps.persist(stored);
	return stored;
}
