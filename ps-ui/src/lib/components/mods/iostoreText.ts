import * as m from '$i18n/messages';
import type { ModError } from '$types';

export function iostoreErrorText(error: ModError): string {
	switch (error.code) {
		case 'not_supported_on_target':
			return m.mods_iostore_error_platform();
		case 'not_convertible':
			return m.mods_iostore_error_not_convertible();
		case 'already_iostore':
			return m.mods_iostore_error_already();
		case 'tool_unavailable':
			return m.mods_iostore_error_tool({ message: error.message });
		case 'conversion_failed':
			return m.mods_iostore_error_failed({ message: error.message });
		case 'mod_not_in_profile':
			return m.mods_iostore_error_not_in_profile();
		case 'remote_denied':
		case 'desktop_only':
			return m.mods_iostore_error_local();
		default:
			return error.message;
	}
}
