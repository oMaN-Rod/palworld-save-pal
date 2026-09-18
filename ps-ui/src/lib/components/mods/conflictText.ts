import * as m from '$i18n/messages';
import type { ModConflict, ModError, ModRef } from '$types';

/** A library mod's display name, or an untracked pak's file name. */
export function refLabel(
	ref: Pick<ModRef, 'mod_id' | 'file'>,
	name: (modId: string) => string
): string {
	return ref.mod_id === null ? m.mods_conflict_untracked({ file: ref.file }) : name(ref.mod_id);
}

export function conflictSummary(conflict: ModConflict, name: (modId: string) => string): string {
	switch (conflict.kind) {
		case 'missing_dependency':
			return conflict.source === 'mod_type'
				? m.mods_conflict_missing_framework({
						mod: name(conflict.mod_id),
						dependency: conflict.dependency
					})
				: m.mods_conflict_missing_package({
						mod: name(conflict.mod_id),
						dependency: conflict.dependency
					});
		case 'gamepass_pak_incompatible':
			return m.mods_conflict_gamepass({ mod: name(conflict.mod_id), count: conflict.files.length });
		case 'pak_overlap':
			return m.mods_conflict_pak_overlap({
				mods: conflict.paks.map((pak) => refLabel(pak, name)).join(', '),
				winner: refLabel(conflict.winner, name),
				count: conflict.asset_count
			});
		case 'palschema_row':
			return m.mods_conflict_palschema_row({
				key: conflict.key,
				mods: conflict.mods.map((entry) => refLabel(entry, name)).join(', ')
			});
	}
}

export function unreadableReasonText(reason: string): string {
	switch (reason) {
		case 'iostore':
			return m.mods_conflict_unreadable_iostore();
		case 'encrypted':
			return m.mods_conflict_unreadable_encrypted();
		case 'unsupported_version':
			return m.mods_conflict_unreadable_version();
		case 'not_a_pak':
			return m.mods_conflict_unreadable_not_a_pak();
		case 'malformed':
			return m.mods_conflict_unreadable_malformed();
		case 'invalid_json':
			return m.mods_conflict_unreadable_invalid_json();
		case 'io':
		default:
			return m.mods_conflict_unreadable_io();
	}
}

export function conflictErrorText(error: ModError): string {
	switch (error.code) {
		case 'no_active_profile':
			return m.mods_conflict_error_no_profile();
		default:
			return error.message;
	}
}
