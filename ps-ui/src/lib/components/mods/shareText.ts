import * as m from '$i18n/messages';
import type { ModError } from '$types';
import { profileErrorText } from './profileText';

export function exportErrorText(error: ModError): string {
	switch (error.code) {
		case 'invalid_path':
			return m.mods_export_invalid_path();
		case 'desktop_only':
			return m.mods_export_desktop_only();
		case 'export_failed':
			return m.mods_export_failed({ reason: String(error.reason ?? error.message) });
		case 'remote_denied':
			return m.mods_export_remote();
		case 'profile_not_found':
		case 'target_not_found':
			return profileErrorText(error);
		default:
			return error.message;
	}
}

export function importErrorText(error: ModError): string {
	const reason = String(error.reason ?? error.message);
	switch (error.code) {
		case 'invalid_path':
			return m.mods_import_invalid_path();
		case 'invalid_archive':
			return m.mods_import_invalid_archive();
		case 'unsupported_format':
			return m.mods_import_unsupported_format();
		case 'invalid_name':
		case 'target_not_found':
			return profileErrorText(error);
		case 'io':
			return m.mods_upload_read_failed({ message: error.message });
		case 'import_failed':
			return error.profile_id
				? m.mods_import_failed_partial({ reason })
				: m.mods_import_failed({ reason });
		case 'desktop_only':
			return m.mods_import_desktop_only();
		case 'remote_denied':
			return m.mods_import_remote_denied();
		default:
			return error.message;
	}
}

export function missingReasonText(reason: string): string {
	switch (reason) {
		case 'not_in_library':
			return m.mods_import_missing_not_in_library();
		case 'id_mismatch':
			return m.mods_import_missing_id_mismatch();
		case 'extract_failed':
			return m.mods_import_missing_extract_failed();
		case 'io_error':
			return m.mods_import_missing_io_error();
		case 'nothing_routed':
			return m.mods_import_missing_nothing_routed();
		case 'library_error':
			return m.mods_import_missing_library_error();
		case 'already_managed':
			return m.mods_import_missing_already_managed();
		default:
			return m.mods_import_missing_install_failed({ code: reason });
	}
}

export function disabledText(code: string): string {
	switch (code) {
		case 'not_supported_on_target':
			return m.mods_import_disabled_not_supported();
		case 'not_subscribed_on_target':
			return m.mods_panel_error_not_subscribed();
		default:
			return code;
	}
}
