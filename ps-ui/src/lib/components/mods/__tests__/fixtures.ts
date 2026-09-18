import type { ApplyResult, LibraryMod, ModTarget, ModVersion, PlanOp } from '$types';

export const noOps: Record<PlanOp, number> = {
	keep: 0,
	reattribute: 0,
	replace: 0,
	preserve: 0,
	add: 0,
	move: 0,
	remove: 0,
	remove_preserve: 0
};

export function modTarget(overrides: Partial<ModTarget> = {}): ModTarget {
	return {
		id: 'client-abc',
		kind: 'client',
		server_id: null,
		name: 'Palworld',
		root_path: 'C:/Steam/steamapps/common/Palworld',
		platform: 'win64',
		ue4ss_mode: 'none',
		layout_overrides: null,
		detected: null,
		last_scanned_at: null,
		layout: null,
		...overrides
	};
}

export function modVersion(overrides: Partial<ModVersion> = {}): ModVersion {
	return {
		id: 'v1',
		mod_id: 'mod-a',
		version: '1.0.0',
		archive_path: null,
		library_dir: 'C:/lib/mod-a/v1',
		source_ref: '{}',
		installed_at: '2026-09-01T10:00:00Z',
		is_current: true,
		size_bytes: 1024,
		manifest: null,
		...overrides
	};
}

export function libraryMod(overrides: Partial<LibraryMod> = {}): LibraryMod {
	return {
		id: 'mod-a',
		name: 'Alpha',
		custom_name: null,
		mod_type: 'ue4ss',
		author: null,
		summary: null,
		source_kind: 'local',
		source_ref: '{}',
		nexus_mod_id: null,
		ignored_version: null,
		notes: null,
		created_at: '2026-09-01T10:00:00Z',
		updated_at: '2026-09-01T10:00:00Z',
		versions: [modVersion()],
		current_version_id: 'v1',
		...overrides
	};
}

export function applyResult(overrides: Partial<ApplyResult> = {}): ApplyResult {
	return {
		target_id: 'server-1',
		request_id: 'req-1',
		mid_apply: false,
		failed: [],
		preserved: [],
		backup_dir: null,
		needs_attention: [],
		counts: { ...noOps },
		new_copies: [],
		skipped_new_copies: [],
		...overrides
	};
}
