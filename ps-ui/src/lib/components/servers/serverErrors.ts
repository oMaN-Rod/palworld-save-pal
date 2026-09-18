import * as m from '$i18n/messages';
import type { ModError } from '$types';

export type ServerAction = 'update' | 'start' | 'delete';

const applyInProgress: Record<ServerAction, () => string> = {
	update: m.servers_update_apply_in_progress,
	start: m.servers_start_apply_in_progress,
	delete: m.servers_delete_apply_in_progress
};

export function serverErrorText(error: ModError, action: ServerAction): string {
	switch (error.code) {
		case 'server_state_unknown':
			return m.servers_state_unknown();
		case 'relocation_pending':
			return m.servers_relocation_pending();
		case 'apply_in_progress':
			return applyInProgress[action]();
		case 'container_create_failed':
			return m.servers_container_create_failed({ message: error.message });
		default:
			return error.message;
	}
}
