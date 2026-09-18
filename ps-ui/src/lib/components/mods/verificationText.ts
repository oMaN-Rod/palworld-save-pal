import * as m from '$i18n/messages';
import type { VerificationStatus } from '$types';

const verificationLabels: Record<VerificationStatus, () => string> = {
	verified: m.mods_verify_verified,
	missing: m.mods_verify_missing,
	unknown: m.mods_verify_unknown,
	unexpected: m.mods_verify_unexpected
};

export function verificationLabel(status: VerificationStatus): string {
	return verificationLabels[status]();
}

export function instanceErrorText(code: string | undefined, message: string): string {
	switch (code) {
		case 'target_not_found':
			return m.mods_live_error_target();
		case 'not_found':
			return m.mods_live_error_instance();
		case 'remote_denied':
			return m.mods_live_error_remote();
		default:
			return message;
	}
}
