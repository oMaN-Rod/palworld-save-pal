import * as m from '$i18n/messages';
import type { ModError } from '$types';
import { platformLabel } from './targets';

export function launchErrorText(error: ModError): string {
	switch (error.code) {
		case 'target_locked':
			return m.mods_launch_running();
		case 'apply_failed':
			return m.mods_launch_apply_failed();
		case 'world_profile_other_target':
			return m.mods_launch_other_target();
		case 'unsupported_platform':
			return m.mods_launch_unsupported_platform({
				platform: platformLabel(String(error.platform ?? ''))
			});
		case 'launch_unavailable':
			return m.mods_launch_unavailable();
		case 'launch_failed':
			return m.mods_launch_failed({ reason: String(error.reason ?? error.message) });
		case 'desktop_only':
		case 'remote_denied':
			return m.mods_launch_desktop_only();
		case 'not_supported_on_target':
			return m.mods_launch_not_a_client();
		case 'target_not_found':
			return m.mods_list_target_missing();
		default:
			return error.message;
	}
}
