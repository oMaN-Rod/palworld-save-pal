export interface ModError {
	code: string;
	message: string;
	[detail: string]: unknown;
}

/** The mod, version or adoption candidate a refused request named, echoed by the server beside `error`. */
export interface RefusalSubject {
	mod_id?: string;
	version_id?: string;
	candidate_name?: string;
	source?: string;
	root?: string;
}

export type RecordedModError = ModError & { target_id?: string; subject?: RefusalSubject };

export interface TargetLayoutJson {
	ue4ss_mods_dir: string | null;
	palschema_mods_dir: string | null;
	paks_mods_dir: string;
	logicmods_dir: string;
	nativemods_dir: string | null;
	workshop_local_dir: string | null;
	mods_txt: string | null;
	palmodsettings_ini: string | null;
}

export interface ModTarget {
	id: string;
	kind: 'client' | 'server';
	server_id: number | null;
	name: string;
	root_path: string;
	platform: string;
	ue4ss_mode: string;
	layout_overrides: unknown;
	detected: { hazards?: string[] } | null;
	last_scanned_at: string | null;
	layout: TargetLayoutJson | null;
}

export interface DetectedInstall {
	root: string;
	platform: string;
	ue4ss_mode: string;
	hazards: string[];
	source: string;
}

export type ModType =
	| 'ue4ss'
	| 'palschema'
	| 'pak'
	| 'logicmods'
	| 'nativedll'
	| 'workshop'
	| 'hybrid'
	| 'framework';

export type RouteKind =
	| 'ue4ss'
	| 'palschema'
	| 'pak'
	| 'logicmods'
	| 'nativedll'
	| 'workshop'
	| 'companion'
	| 'passthrough'
	| 'framework';

export interface FileRoute {
	archive_path: string;
	rel_path: string;
	kind: RouteKind;
}

export type Decision =
	| { kind: 'pak_destination'; file: string; default: RouteKind }
	| { kind: 'multiple_ue4ss_roots'; roots: string[] }
	| { kind: 'unplaced_files'; files: string[] }
	| { kind: 'name_conflict'; proposed: string; existing_mod_id: string }
	| { kind: 'nexus_variant'; existing_mod_id: string; file_name: string };

/** Guessed from the archive and its `Info.json`, not authoritative provenance. */
export interface SourceHint {
	nexus_mod_id?: number | null;
	nexus_file_id?: string | null;
	workshop_package?: string | null;
	version?: string | null;
	author?: string | null;
	/** `false` when no `InstallRule` sets `IsServer`, so a dedicated server deploys nothing. */
	server_capable?: boolean | null;
}

export type Platform = 'win64' | 'wingdk' | 'linux' | 'mac';

export interface InstallManifest {
	folder_name: string;
	display_name: string;
	mod_type: ModType;
	version: string;
	routes: FileRoute[];
	decisions: Decision[];
	platform_filtered: Platform | null;
	source: SourceHint;
}

export interface ModVersion {
	id: string;
	mod_id: string;
	version: string;
	archive_path: string | null;
	library_dir: string;
	source_ref: string;
	installed_at: string;
	is_current: boolean;
	size_bytes: number | null;
	/** `null` when the stored manifest no longer parses. */
	manifest: InstallManifest | null;
}

export interface LibraryMod {
	id: string;
	name: string;
	custom_name: string | null;
	mod_type: string;
	author: string | null;
	summary: string | null;
	source_kind: string;
	source_ref: string;
	nexus_mod_id: number | null;
	ignored_version: string | null;
	notes: string | null;
	created_at: string;
	updated_at: string;
	versions: ModVersion[];
	current_version_id: string | null;
}

export interface ProfileMod {
	profile_id: string;
	mod_id: string;
	/** `null` follows the mod's current version. */
	mod_version_id: string | null;
	enabled: boolean;
	load_order: number;
}

export interface ProfileWorld {
	world_key: string;
	world_name: string;
}

export interface ModProfile {
	id: string;
	target_id: string;
	name: string;
	is_active: boolean;
	is_default: boolean;
	mods: ProfileMod[];
	ue4ss_control_mode: string;
	force_order_ue4ss: boolean;
	force_order_palschema: boolean;
	created_at: string;
	updated_at: string;
	worlds?: ProfileWorld[];
}

export type ReorderKind = 'ue4ss' | 'palschema';

export type Ue4ssControlMode = 'enabled_txt' | 'mods_txt';

export interface ProfileOptions {
	ue4ss_control_mode?: Ue4ssControlMode;
	force_order_ue4ss?: boolean;
	force_order_palschema?: boolean;
}

/** An edit of a profile that is not active replies with a null request id, `pending: false` and a null apply. */
export interface SelectionApply {
	request_id: string | null;
	pending: boolean;
	apply: ApplyResult | null;
}

export interface ProfileRemoveModReply {
	target_id: string;
	profile_id: string;
	mod_id: string;
	removed: true;
}

export interface ProfileRef {
	target_id: string;
	profile_id: string;
}

export interface ReleaseProfilesReply {
	mod_id: string;
	removed: ProfileRef[];
	enabled: ProfileRef[];
}

/** One profile entry a `version_in_use` refusal of `mod_remove` names; the names are the server's. */
export interface ModInUseHolder {
	target_id: string;
	target_name: string;
	profile_id: string;
	profile_name: string;
	enabled: boolean;
}

export interface ModInUseDeployed {
	target_id: string;
	target_name: string;
}

/** A target whose framework slot holds a version of the mod. */
export type ModInUseFramework = ModInUseDeployed;

export interface SaveModProfile {
	profile_id: string;
	profile_name: string;
	target_id: string;
}

/** `world_key` is the directory holding `Level.sav`, in native separators. */
export interface LocalSave {
	path: string;
	name: string;
	save_type: string;
	modified_ms: number;
	world_key: string;
	mod_profile: SaveModProfile | null;
}

export interface LaunchReply {
	target_id: string;
	world_key: string | null;
	profile_id: string;
	activated: boolean;
	apply: ApplyResult | null;
	launched: true;
}

export type UploadPurpose = 'install' | 'import';

export type UploadStage = 'hashing' | 'uploading' | 'finishing' | 'done' | 'failed';

/** `uploadId` is null until the server accepts the upload; `sent` counts bytes the server confirmed. */
export interface UploadStatus {
	purpose: UploadPurpose;
	targetId: string;
	name: string;
	size: number;
	sent: number;
	stage: UploadStage;
	uploadId: string | null;
	path: string | null;
	error: ModError | null;
}

export interface ExportReply {
	target_id: string;
	profile_id: string;
	path: string;
	entries: number;
	archives_included: number;
	missing_archives: string[];
	unresolved: string[];
}

export interface ImportedVersion {
	mod_id: string;
	version_id: string;
}

/** `reason` is `not_in_library`, `id_mismatch`, `extract_failed`, or the install error code of an included archive. */
export interface ImportMissing {
	mod_id: string;
	name: string;
	version: string;
	reason: string;
}

export interface ImportDisabled {
	mod_id: string;
	code: string;
}

export interface ImportFramework {
	framework: string;
	mod_id: string;
	version: string;
	in_library: boolean;
}

export interface ImportReply {
	target_id: string;
	path: string;
	profile: ModProfile;
	pinned: string[];
	following_current: string[];
	installed: ImportedVersion[];
	missing: ImportMissing[];
	disabled: ImportDisabled[];
	frameworks: ImportFramework[];
}

export type PlanOp =
	| 'keep'
	| 'reattribute'
	| 'replace'
	| 'preserve'
	| 'add'
	| 'move'
	| 'remove'
	| 'remove_preserve';

export type PlanEntry =
	| { op: 'keep'; path: string }
	| { op: 'reattribute'; path: string; mod_version_id: string; rel_path: string }
	| {
			op: 'replace' | 'preserve' | 'add';
			path: string;
			source: string;
			expected_hash: string;
			mod_version_id: string;
			rel_path: string;
			role: string;
	  }
	| {
			op: 'move';
			from: string;
			to: string;
			mod_version_id: string;
			rel_path: string;
			hash: string;
			role: string;
			recorded_hash: string;
	  }
	| { op: 'remove'; path: string }
	| { op: 'remove_preserve'; path: string; hash: string };

export interface TargetPlan {
	profile_id: string;
	/** Always carries all eight ops, zero-filled. */
	counts: Record<PlanOp, number>;
	entries: PlanEntry[];
}

export interface NeedsAttention {
	path: string;
	reason: 'unreadable' | 'drift';
}

/**
 * `request_id` correlates this reply with its `mod_progress` frames; `backup_dir` names the backup set.
 * A partial replace reports `error.code === 'replace_partial'` with `moved` and a `backup_dir`.
 */
export interface ApplyResult {
	target_id: string;
	request_id: string;
	mid_apply: boolean;
	failed: string[];
	preserved: string[];
	backup_dir: string | null;
	error?: ModError;
	needs_attention: NeedsAttention[];
	counts: Record<PlanOp, number>;
	new_copies: string[];
	skipped_new_copies: string[];
}

export interface ModProgress {
	request_id: string;
	target_id: string;
	stage: string;
	pct: number;
	message: string;
}

export type FileState =
	| 'managed_intact'
	| 'managed_drifted'
	| 'managed_missing'
	| 'managed_unreadable'
	| 'unmanaged';

export interface ScannedFile {
	path: string;
	kind: RouteKind | null;
	state: FileState;
	mod_version_id: string | null;
}

export interface AdoptionCandidate {
	name: string;
	kind: RouteKind;
	root: string;
	/** Empty for a Steam-subscribed package, whose files Steam owns. */
	files: string[];
	enabled: boolean;
	source: 'local' | 'steam_subscribed';
}

/** A candidates-only report leaves `files`, `drifted`, `missing` and `unreadable` empty. */
export interface ScanReport {
	target_id: string;
	files: ScannedFile[];
	candidates: AdoptionCandidate[];
	drifted: string[];
	missing: string[];
	unreadable: string[];
	warnings: string[];
}

export interface BackupEntry {
	original_path: string;
	backup_key: string;
	hash: string;
}

export interface BackupSet {
	name: string;
	size_bytes: number;
	entries: BackupEntry[];
}

/** `reason` is `occupied` or `changed` for the expected cases, otherwise a code or an I/O error message. */
export interface RestoreSkip {
	path: string;
	reason: string;
}

export interface RestoreReply {
	target_id: string;
	set: string;
	restored: string[];
	skipped: RestoreSkip[];
}

/** `seq` is the install request it answered, counted per target. */
export interface InstallRecord {
	seq: number;
	path: string;
	mod_id: string;
	version_id: string;
	enable_error?: ModError;
}

export interface AnalyzeReply {
	target_id: string;
	path: string;
	manifest: InstallManifest;
}

export interface PendingDecisions {
	path: string;
	decisions: Decision[];
}

export type FrameworkKey = 'ue4ss' | 'palschema' | 'amity';

export interface FrameworkLibraryEntry {
	mod_version_id: string;
	version: string;
	display: string;
	installed_at: string;
	is_current: boolean;
}

export interface FrameworkEntry {
	key: FrameworkKey;
	name: string;
	installed: {
		present: boolean;
		version: string | null;
		managed: boolean;
		mod_version_id: string | null;
	};
	library: FrameworkLibraryEntry[];
	latest: { version: string; display: string; in_library: boolean } | null;
	latest_error: { code: string; message: string } | null;
	update_available: boolean;
}

export interface FrameworkStatus {
	target_id: string;
	ue4ss_mode: string;
	hazards: { code: string; paths: string[] }[];
	frameworks: FrameworkEntry[];
}

export interface FrameworkInstallReply extends SelectionApply {
	target_id: string;
	key: FrameworkKey;
	mod_version_id: string;
	version: string;
	display: string;
	installed_new: boolean;
}

export interface FrameworkRemoveReply extends SelectionApply {
	target_id: string;
	key: FrameworkKey;
	removed: boolean;
	also_removed: FrameworkKey[];
}

export interface HazardRemoveReply {
	target_id: string;
	hazard: string;
	moved: string[];
	backup_dir: string;
}

export type PakSource = 'library' | 'disk';

/** `mod_id` is null for a pak found on disk that no library mod accounts for; `path` is set for disk paks only. */
export interface ModRef {
	mod_id: string | null;
	file: string;
	source: PakSource;
	path: string | null;
}

export type ModConflict =
	| {
			kind: 'missing_dependency';
			mod_id: string;
			dependency: string;
			source: 'mod_type' | 'workshop_info';
	  }
	| { kind: 'gamepass_pak_incompatible'; mod_id: string; files: string[] }
	| { kind: 'pak_overlap'; paks: ModRef[]; winner: ModRef; assets: string[]; asset_count: number }
	| { kind: 'palschema_row'; key: string; mods: ModRef[] };

export interface UnreadableFile extends ModRef {
	reason: string;
}

export interface ConflictReport {
	target_id: string;
	profile_id: string;
	platform: string;
	conflicts: ModConflict[];
	unreadable: UnreadableFile[];
}

export type VerificationStatus = 'missing' | 'unexpected' | 'verified' | 'unknown';

export interface ModVerification {
	mod_id: string | null;
	name: string;
	kind: string;
	status: VerificationStatus;
}

export interface IostoreConvertReply extends SelectionApply {
	target_id: string;
	mod_id: string;
	mod_version_id: string;
	version: string;
	converted: string[];
	reused: boolean;
}

export interface TargetVerification {
	target_id: string;
	instance_id: string | null;
	live: boolean;
	checked_at: string;
	build_info: Record<string, string> | null;
	resolution: { complete: boolean; missing: string[] };
	status: ModVerification[];
}
