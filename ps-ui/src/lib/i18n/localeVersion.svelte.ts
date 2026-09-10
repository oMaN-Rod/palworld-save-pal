let version = $state(0);

// Paraglide message accessors read module-scoped state, so a locale change does
// not re-render on its own. The layout keys on this counter to force it.
export const localeState = {
	get version(): number {
		return version;
	}
};

export function bumpLocaleVersion(): void {
	version += 1;
}
