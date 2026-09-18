import * as m from '$i18n/messages';
import type { ModError } from '$types';

export function profileErrorText(error: ModError): string {
	switch (error.code) {
		case 'invalid_name':
			return m.mods_profile_invalid_name();
		case 'name_taken':
			return m.mods_profile_name_taken();
		case 'default_profile':
			return m.mods_profile_default_undeletable();
		case 'profile_not_found':
			return m.mods_profile_not_found();
		case 'no_active_profile':
			return m.mods_apply_no_active_profile();
		case 'target_not_found':
			return m.mods_list_target_missing();
		case 'invalid_kind':
		case 'invalid_order':
			return m.mods_order_stale();
		case 'invalid_option':
			return m.mods_order_invalid_option();
		default:
			return error.message;
	}
}

export function worldLinkErrorText(error: ModError): string {
	const message = error.code === 'profile_not_found' ? m.mods_profile_not_found() : error.message;
	return m.mods_worlds_link_failed({ world: String(error.world_name ?? ''), message });
}
