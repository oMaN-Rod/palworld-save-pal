import * as m from '$i18n/messages';
import type { ModError } from '$types';

export const frameworkLabels: Record<string, string> = {
	ue4ss: 'UE4SS',
	palschema: 'PalSchema',
	amity: 'Amity'
};

/** `names` are the resolved dependent names; omitted, the raw ids from `error.dependents` are joined instead. */
export function frameworkErrorText(error: ModError, names?: string[]): string {
	switch (error.code) {
		case 'not_supported_on_target':
			return m.mods_framework_error_docker();
		case 'framework_conflict':
			return m.mods_framework_error_conflict();
		case 'framework_required': {
			const dependents = names ?? (error.dependents as string[]) ?? [];
			if (dependents.length === 0 && typeof error.framework === 'string') {
				return m.mods_framework_error_requires({
					framework: frameworkLabels[error.framework] ?? error.framework
				});
			}
			return m.mods_framework_error_required({ dependents: dependents.join(', ') });
		}
		case 'mods_dir_mismatch':
			return m.mods_framework_error_mods_dir();
		case 'framework_not_installed':
			return m.mods_framework_error_not_installed();
		case 'rate_limited':
			return m.mods_framework_error_rate_limited();
		case 'no_release_asset':
		case 'framework_unavailable':
			return m.mods_framework_error_unavailable();
		case 'download_too_large':
		case 'network':
			return m.mods_framework_error_network({ message: error.message });
		case 'invalid_framework_archive':
		case 'extract_failed':
			return m.mods_framework_error_archive();
		case 'hazard_not_found':
			return m.mods_hazard_error_gone();
		default:
			return error.message;
	}
}
