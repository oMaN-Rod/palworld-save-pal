import * as m from '$i18n/messages';
import { getLocale } from '$i18n/runtime';
import type { ApplyResult, LibraryMod, ModError, PlanOp } from '$types';
import { modTypeLabel } from './modLabels';
import { displayName } from './modList';

type Counts = Partial<Record<PlanOp, number>>;

interface ConfirmModal {
	showConfirmModal(options: {
		title?: string;
		message?: string;
		confirmText?: string;
		cancelText?: string;
	}): Promise<boolean>;
}

/** `preserve` is left out: an edited file plans as `preserve` again after every apply. */
export function pendingTotal(counts: Counts): number {
	return (Object.entries(counts) as [PlanOp, number | undefined][])
		.filter(([op]) => op !== 'keep' && op !== 'preserve')
		.reduce((sum, [, count]) => sum + (count ?? 0), 0);
}

export function pendingParts(counts: Counts): string[] {
	const count = (...ops: PlanOp[]) => ops.reduce((sum, op) => sum + (counts[op] ?? 0), 0);
	const parts: [number, (inputs: { count: number }) => string][] = [
		[count('add'), m.mods_apply_to_add],
		[count('replace', 'reattribute'), m.mods_apply_to_update],
		[count('move'), m.mods_apply_to_move],
		[count('remove'), m.mods_apply_to_remove],
		[count('remove_preserve'), m.mods_apply_kept_edited]
	];
	return parts.filter(([total]) => total > 0).map(([total, text]) => text({ count: total }));
}

export function appliedStats(counts: Counts): { label: string; count: number }[] {
	const count = (...ops: PlanOp[]) => ops.reduce((sum, op) => sum + (counts[op] ?? 0), 0);
	const stats: [number, () => string][] = [
		[count('add'), m.mods_apply_result_added],
		[count('replace', 'reattribute'), m.mods_apply_result_updated],
		[count('move'), m.mods_apply_result_moved],
		[count('remove', 'remove_preserve'), m.mods_apply_result_removed]
	];
	return stats
		.filter(([total]) => total > 0)
		.map(([total, label]) => ({ label: label(), count: total }));
}

export function isFailed(result: ApplyResult): boolean {
	return result.error !== undefined || result.mid_apply;
}

/** A plain success needs no more than a toast; anything else is worth reading. */
export function isNotable(result: ApplyResult): boolean {
	return (
		isFailed(result) ||
		result.needs_attention.length > 0 ||
		result.preserved.length > 0 ||
		result.new_copies.length > 0 ||
		result.skipped_new_copies.length > 0 ||
		result.backup_dir !== null
	);
}

export function joinList(items: string[], type: 'conjunction' | 'unit'): string {
	return new Intl.ListFormat(getLocale(), { type, style: 'long' }).format(items);
}

export function stringList(value: unknown): string[] {
	return Array.isArray(value)
		? value.filter((item): item is string => typeof item === 'string')
		: [];
}

export function versionName(mods: LibraryMod[], versionId: string): string {
	const mod = mods.find((entry) => entry.versions.some((version) => version.id === versionId));
	return mod ? displayName(mod) : versionId;
}

export function confirmReplace(modal: ConfirmModal, remote: boolean): Promise<boolean> {
	return modal.showConfirmModal({
		title: m.mods_apply_replace_title(),
		message: remote ? m.mods_apply_replace_message_remote() : m.mods_apply_replace_message(),
		confirmText: m.mods_apply_replace(),
		cancelText: m.mods_panel_cancel()
	});
}

export function errorText(error: ModError, mods: LibraryMod[]): string {
	switch (error.code) {
		case 'target_locked':
			return m.mods_apply_target_locked();
		case 'apply_in_progress':
			return m.mods_apply_in_progress();
		case 'not_supported_on_target':
			return error.kind === 'workshop'
				? m.mods_apply_not_supported_workshop()
				: m.mods_list_not_supported({ kind: modTypeLabel(String(error.kind ?? '')) });
		case 'no_active_profile':
			return m.mods_apply_no_active_profile();
		case 'destination_conflict':
			return m.mods_apply_conflict({
				mods: joinList(
					stringList(error.mod_version_ids).map((id) => versionName(mods, id)),
					'conjunction'
				),
				path: String(error.path ?? '')
			});
		case 'unmanaged_occupant':
			return m.mods_apply_occupants();
		case 'replace_partial':
			return m.mods_apply_replace_partial();
		default:
			return error.message;
	}
}
