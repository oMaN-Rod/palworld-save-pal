import * as m from '$i18n/messages';
import { getModsState, getServerState, getToastState } from '$states';
import type {
	ApplyResult,
	BackupSet,
	ConflictReport,
	Decision,
	DetectedInstall,
	ExportReply,
	FrameworkInstallReply,
	FrameworkRemoveReply,
	FrameworkStatus,
	HazardRemoveReply,
	ImportReply,
	InstallManifest,
	IostoreConvertReply,
	LaunchReply,
	LibraryMod,
	LocalSave,
	ModError,
	ModProfile,
	ModProgress,
	ModTarget,
	PlanEntry,
	PlanOp,
	ProfileRemoveModReply,
	RefusalSubject,
	ReleaseProfilesReply,
	RestoreSkip,
	ScanReport,
	SelectionApply,
	TargetVerification
} from '$types';
import { MessageType } from '$types';
import type { WSMessageHandler } from '$ws/types';

interface Reply {
	canceled?: boolean;
	error?: unknown;
	target_id?: string;
	mod_id?: string;
	version_id?: string;
	candidate_name?: string;
	source?: string;
	root?: string;
}

/** Refusals the mod list renders under the row they name, so a toast would repeat them. */
const inlineRefusals = new Set<string>([
	MessageType.PROFILE_SET_MOD,
	MessageType.MOD_REMOVE,
	MessageType.MOD_VERSION_DELETE,
	MessageType.MOD_VERSION_SET_CURRENT,
	MessageType.MOD_IOSTORE_CONVERT,
	MessageType.PROFILE_REMOVE_MOD,
	MessageType.MOD_RELEASE_PROFILES,
	MessageType.NEXUS_DOWNLOAD,
	MessageType.MOD_UPDATE_IGNORE,
	MessageType.NEXUS_HANDLER_REGISTER,
	MessageType.NEXUS_SEARCH,
	MessageType.NEXUS_MOD_FILES,
	MessageType.NEXUS_ACCOUNT_GET,
	MessageType.NEXUS_KEY_SET
]);

/** Refusals a target's panel or the install modal renders; one without a target has nowhere to show. */
const panelRefusals = new Set<string>([
	MessageType.MOD_ANALYZE,
	MessageType.MOD_INSTALL,
	MessageType.MOD_ADOPT,
	MessageType.MOD_TARGET_SCAN,
	MessageType.MOD_BACKUP_LIST,
	MessageType.MOD_BACKUP_RESTORE,
	MessageType.MOD_BACKUP_DELETE,
	MessageType.PROFILE_CREATE,
	MessageType.PROFILE_RENAME,
	MessageType.PROFILE_DELETE,
	MessageType.PROFILE_ACTIVATE,
	MessageType.PROFILE_REORDER,
	MessageType.PROFILE_SET_OPTIONS,
	MessageType.GAME_LAUNCH,
	MessageType.PROFILE_EXPORT,
	MessageType.PROFILE_IMPORT,
	MessageType.FRAMEWORK_STATUS,
	MessageType.FRAMEWORK_INSTALL,
	MessageType.FRAMEWORK_REMOVE,
	MessageType.FRAMEWORK_HAZARD_REMOVE,
	MessageType.MOD_CONFLICTS,
	MessageType.MOD_VERIFICATION_GET,
	MessageType.NEXUS_CATEGORIES,
	MessageType.NEXUS_KEY_CLEAR,
	MessageType.NEXUS_HANDLER_STATUS,
	MessageType.MOD_UPDATE_CHECK
]);

const subjectFields = ['mod_id', 'version_id', 'candidate_name', 'source', 'root'] as const;

function subjectOf(data: Reply): RefusalSubject | undefined {
	const subject: RefusalSubject = {};
	for (const field of subjectFields) {
		const value = data[field];
		if (typeof value === 'string') subject[field] = value;
	}
	return Object.keys(subject).length === 0 ? undefined : subject;
}

/** A refusal from outside the mods handlers can carry a bare string instead of `{ code, message }`. */
function errorOf(data: Reply): ModError | undefined {
	const error = data.error;
	if (error === undefined || error === null) return undefined;
	if (typeof error === 'object') return error as ModError;
	return { code: 'unknown', message: String(error) };
}

function refuse(
	type: MessageType,
	error: ModError,
	targetId?: string,
	toast = true,
	subject?: RefusalSubject
): void {
	getModsState().recordRefusal(type, error, targetId, { subject });
	if (toast) getToastState().add(error.message, m.mods_panel_title(), 'error');
}

function toastsRefusal(type: MessageType, targetId: string | undefined): boolean {
	if (inlineRefusals.has(type)) return false;
	if (panelRefusals.has(type)) return targetId === undefined;
	return true;
}

/** True when the reply was a cancelled dialog or a refusal, both already dealt with. */
export function settled(
	type: MessageType,
	data: Reply,
	targetId?: string,
	options: { toast?: boolean } = {}
): boolean {
	if (data.canceled) return true;
	const error = errorOf(data);
	const subject = subjectOf(data);
	if (error) {
		refuse(type, error, targetId, options.toast ?? toastsRefusal(type, targetId), subject);
		return true;
	}
	getModsState().clearLastError(type, targetId, subject);
	return false;
}

export const modTargetListHandler: WSMessageHandler = {
	type: MessageType.MOD_TARGET_LIST,
	async handle(data: Reply & { targets?: ModTarget[] }) {
		if (settled(MessageType.MOD_TARGET_LIST, data)) return;
		const state = getModsState();
		state.targets = data.targets ?? [];
		state.targetsLoaded = true;
	}
};

export const modTargetDetectHandler: WSMessageHandler = {
	type: MessageType.MOD_TARGET_DETECT,
	async handle(data: Reply & { detected?: DetectedInstall[] }) {
		const state = getModsState();
		state.detecting = false;
		if (settled(MessageType.MOD_TARGET_DETECT, data)) return;
		state.detected = data.detected ?? [];
	}
};

export const modTargetAddHandler: WSMessageHandler = {
	type: MessageType.MOD_TARGET_ADD,
	async handle(data: Reply & { root_path?: string; target?: ModTarget }) {
		const state = getModsState();
		if (settled(MessageType.MOD_TARGET_ADD, data, undefined, { toast: false })) {
			state.answerAdd(null);
			return;
		}
		const target = data.target;
		if (target) {
			state.targets = [...state.targets.filter((entry) => entry.id !== target.id), target];
		}
		state.answerAdd(target?.id ?? null);
	}
};

export const modTargetRemoveHandler: WSMessageHandler = {
	type: MessageType.MOD_TARGET_REMOVE,
	async handle(data: Reply & { target_id: string; removed?: boolean }) {
		if (settled(MessageType.MOD_TARGET_REMOVE, data, data.target_id)) return;
		getModsState().forgetTarget(data.target_id);
	}
};

export const modListHandler: WSMessageHandler = {
	type: MessageType.MOD_LIST,
	async handle(data: Reply & { mods?: LibraryMod[] }) {
		if (settled(MessageType.MOD_LIST, data)) return;
		getModsState().mods = data.mods ?? [];
	}
};

export const profileListHandler: WSMessageHandler = {
	type: MessageType.PROFILE_LIST,
	async handle(data: Reply & { target_id: string; profiles?: ModProfile[] }) {
		if (settled(MessageType.PROFILE_LIST, data, data.target_id)) return;
		const state = getModsState();
		state.profiles = { ...state.profiles, [data.target_id]: data.profiles ?? [] };
	}
};

/** The apply bar shows a result that carries a request id, so its refusal is not toasted. */
function recordApply(result: ApplyResult): void {
	const state = getModsState();
	state.recordApply(result);
	state.rescan(result.target_id);
	if (result.error) refuse(MessageType.PROFILE_APPLY, result.error, result.target_id, false);
	else state.clearLastError(MessageType.PROFILE_APPLY, result.target_id);
	if (!result.error && !result.mid_apply) settleRelocation(result.target_id);
}

/**
 * A clean apply makes a failed move's result and error stale, but only starting the server closes
 * the move. Once the mods have moved, what is stored is about setting the server up, so it stays.
 */
function settleRelocation(targetId: string): void {
	const match = /^server-(\d+)$/.exec(targetId);
	if (!match) return;
	const servers = getServerState();
	const serverId = Number(match[1]);
	const pending = servers.relocationPending[serverId];
	if (pending === undefined || servers.relocationMoved[serverId]) return;
	if (pending !== true || servers.relocationError[serverId]) servers.setRelocation(serverId, null);
}

type SelectionReply = Partial<SelectionApply>;

/** An edit of a profile that is not active deploys nothing, so it leaves the target's pending state alone. */
function applySelection(targetId: string, reply: SelectionReply): void {
	const state = getModsState();
	if (reply.pending !== true && reply.request_id == null && reply.apply == null) return;
	state.pending = { ...state.pending, [targetId]: reply.pending === true };
	if (reply.pending && reply.request_id) state.clearProgress(reply.request_id);
	const apply = reply.apply;
	if (!apply) return;
	if (apply.request_id !== undefined) return recordApply(apply);
	const error = errorOf(apply as Reply);
	if (error) refuse(MessageType.PROFILE_APPLY, error, targetId, false);
}

export const profileSetModHandler: WSMessageHandler = {
	type: MessageType.PROFILE_SET_MOD,
	async handle(
		data: Reply &
			SelectionReply & {
				target_id: string;
				profile_id?: string;
				mod_id: string;
				enabled?: boolean;
				mod_version_id?: string | null;
			}
	) {
		const state = getModsState();
		state.finishBusy('settingMod', data.target_id);
		if (settled(MessageType.PROFILE_SET_MOD, data, data.target_id)) return;
		if (data.profile_id !== undefined && typeof data.enabled === 'boolean') {
			state.recordSetMod(
				data.target_id,
				data.profile_id,
				data.mod_id,
				data.enabled,
				data.mod_version_id ?? null
			);
		}
		state.loadProfiles(data.target_id);
		applySelection(data.target_id, data);
		state.plan(data.target_id);
	}
};

export const profileCreateHandler: WSMessageHandler = {
	type: MessageType.PROFILE_CREATE,
	async handle(data: Reply & { target_id: string; name?: string; profile?: ModProfile }) {
		const state = getModsState();
		state.finishBusy('managingProfile', data.target_id);
		if (settled(MessageType.PROFILE_CREATE, data, data.target_id)) return;
		if (!data.profile) return;
		state.upsertProfile(data.profile);
		state.viewProfile(data.target_id, data.profile.id);
	}
};

export const profileRenameHandler: WSMessageHandler = {
	type: MessageType.PROFILE_RENAME,
	async handle(data: Reply & { target_id: string; profile?: ModProfile }) {
		const state = getModsState();
		state.finishBusy('managingProfile', data.target_id);
		if (settled(MessageType.PROFILE_RENAME, data, data.target_id)) {
			if (errorOf(data)) state.loadProfiles(data.target_id);
			return;
		}
		if (data.profile) state.upsertProfile(data.profile);
	}
};

export const profileDeleteHandler: WSMessageHandler = {
	type: MessageType.PROFILE_DELETE,
	async handle(
		data: Reply &
			SelectionReply & {
				target_id: string;
				profile_id: string;
				deleted?: boolean;
				active_profile_id?: string;
			}
	) {
		const state = getModsState();
		state.finishBusy('managingProfile', data.target_id);
		if (settled(MessageType.PROFILE_DELETE, data, data.target_id)) {
			if (errorOf(data)) state.loadProfiles(data.target_id);
			return;
		}
		state.removeProfile(data.target_id, data.profile_id);
		if (data.active_profile_id) state.markActive(data.target_id, data.active_profile_id);
		state.loadProfiles(data.target_id);
		applySelection(data.target_id, data);
		state.plan(data.target_id);
	}
};

export const profileActivateHandler: WSMessageHandler = {
	type: MessageType.PROFILE_ACTIVATE,
	async handle(data: Reply & SelectionReply & { target_id: string; profile_id: string }) {
		const state = getModsState();
		state.finishBusy('activating', data.target_id);
		if (settled(MessageType.PROFILE_ACTIVATE, data, data.target_id)) {
			if (errorOf(data)) state.loadProfiles(data.target_id);
			return;
		}
		state.markActive(data.target_id, data.profile_id);
		state.loadProfiles(data.target_id);
		applySelection(data.target_id, data);
		state.plan(data.target_id);
	}
};

export const profileReorderHandler: WSMessageHandler = {
	type: MessageType.PROFILE_REORDER,
	async handle(
		data: Reply &
			SelectionReply & {
				target_id: string;
				profile_id?: string;
				kind?: string;
				ordered_mod_ids?: string[];
			}
	) {
		const state = getModsState();
		state.finishBusy('ordering', data.target_id);
		if (settled(MessageType.PROFILE_REORDER, data, data.target_id)) {
			if (errorOf(data)) state.loadProfiles(data.target_id);
			return;
		}
		state.loadProfiles(data.target_id);
		applySelection(data.target_id, data);
		state.plan(data.target_id);
	}
};

export const profileSetOptionsHandler: WSMessageHandler = {
	type: MessageType.PROFILE_SET_OPTIONS,
	async handle(data: Reply & SelectionReply & { target_id: string; profile?: ModProfile }) {
		const state = getModsState();
		state.finishBusy('ordering', data.target_id);
		if (settled(MessageType.PROFILE_SET_OPTIONS, data, data.target_id)) {
			if (errorOf(data)) state.loadProfiles(data.target_id);
			return;
		}
		if (data.profile) state.upsertProfile(data.profile);
		applySelection(data.target_id, data);
		state.plan(data.target_id);
	}
};

export const profilePlanHandler: WSMessageHandler = {
	type: MessageType.PROFILE_PLAN,
	async handle(
		data: Reply & {
			target_id: string;
			profile_id: string;
			entries?: PlanEntry[];
			counts: Record<PlanOp, number>;
		}
	) {
		const state = getModsState();
		if (settled(MessageType.PROFILE_PLAN, data, data.target_id, { toast: false })) {
			if (errorOf(data)) state.forgetPlan(data.target_id);
			return;
		}
		state.plans = {
			...state.plans,
			[data.target_id]: {
				profile_id: data.profile_id,
				counts: data.counts,
				entries: data.entries ?? []
			}
		};
	}
};

export const profileApplyHandler: WSMessageHandler = {
	type: MessageType.PROFILE_APPLY,
	async handle(data: Reply & Partial<Omit<ApplyResult, 'error'>> & { target_id?: string }) {
		if (data.canceled) return;
		const state = getModsState();
		const error = errorOf(data);
		if (data.request_id === undefined || data.target_id === undefined) {
			state.applying =
				data.target_id === undefined ? {} : { ...state.applying, [data.target_id]: false };
			if (error) refuse(MessageType.PROFILE_APPLY, error, data.target_id);
			return;
		}
		recordApply({ ...data, error } as ApplyResult);
		state.plan(data.target_id);
		if (error) return;
		state.pending = { ...state.pending, [data.target_id]: false };
		getToastState().add(m.mods_panel_applied(), m.mods_panel_title(), 'success');
	}
};

export const modAnalyzeHandler: WSMessageHandler = {
	type: MessageType.MOD_ANALYZE,
	async handle(data: Reply & { target_id: string; path: string; manifest?: InstallManifest }) {
		const state = getModsState();
		state.finishBusy('analyzing', data.target_id);
		if (settled(MessageType.MOD_ANALYZE, data, data.target_id)) return;
		const manifest = data.manifest;
		if (!manifest) return;
		state.analysis = {
			...state.analysis,
			[data.target_id]: { target_id: data.target_id, path: data.path, manifest }
		};
	}
};

export const modInstallHandler: WSMessageHandler = {
	type: MessageType.MOD_INSTALL,
	async handle(
		data: Reply & {
			target_id: string;
			path: string;
			mod_id?: string;
			version_id?: string;
			manifest?: { display_name?: string };
			needs_decisions?: Decision[];
			enable_error?: ModError;
		}
	) {
		const state = getModsState();
		state.finishBusy('installing', data.target_id);
		if (settled(MessageType.MOD_INSTALL, data, data.target_id)) return;
		if (data.needs_decisions !== undefined) {
			state.needsDecisions = {
				...state.needsDecisions,
				[data.target_id]: { path: data.path, decisions: data.needs_decisions }
			};
			return;
		}
		state.recordInstall(data.target_id, {
			path: data.path,
			mod_id: data.mod_id ?? '',
			version_id: data.version_id ?? '',
			enable_error: data.enable_error
		});
		state.loadLibrary();
		state.loadProfiles(data.target_id);
		state.rescan(data.target_id);
		if (!data.enable_error) state.plan(data.target_id);
	}
};

export const modRemoveHandler: WSMessageHandler = {
	type: MessageType.MOD_REMOVE,
	async handle(data: Reply & { mod_id: string; removed?: boolean }) {
		if (settled(MessageType.MOD_REMOVE, data)) return;
		const state = getModsState();
		state.loadLibrary();
		state.rescanAll();
		state.planLoaded();
	}
};

export const profileRemoveModHandler: WSMessageHandler = {
	type: MessageType.PROFILE_REMOVE_MOD,
	async handle(data: Reply & Partial<ProfileRemoveModReply>) {
		const state = getModsState();
		state.finishBusy('settingMod', data.target_id);
		if (settled(MessageType.PROFILE_REMOVE_MOD, data, data.target_id)) return;
		if (!data.target_id || !data.profile_id || !data.mod_id) return;
		state.recordProfileRemove(data.target_id, data.profile_id, data.mod_id);
		state.loadProfiles(data.target_id);
		state.plan(data.target_id);
	}
};

export const modReleaseProfilesHandler: WSMessageHandler = {
	type: MessageType.MOD_RELEASE_PROFILES,
	async handle(data: Reply & Partial<Omit<ReleaseProfilesReply, 'mod_id'>>) {
		const state = getModsState();
		state.finishBusy('releasing', data.mod_id);
		if (settled(MessageType.MOD_RELEASE_PROFILES, data)) return;
		if (!data.mod_id) return;
		const reply: ReleaseProfilesReply = {
			mod_id: data.mod_id,
			removed: data.removed ?? [],
			enabled: data.enabled ?? []
		};
		state.recordRelease(reply);
		for (const targetId of new Set(reply.removed.map((entry) => entry.target_id))) {
			state.loadProfiles(targetId);
			state.plan(targetId);
		}
	}
};

export const modVersionSetCurrentHandler: WSMessageHandler = {
	type: MessageType.MOD_VERSION_SET_CURRENT,
	async handle(data: Reply & { mod_id: string; version_id: string }) {
		if (settled(MessageType.MOD_VERSION_SET_CURRENT, data)) return;
		const state = getModsState();
		state.loadLibrary();
		state.planLoaded();
	}
};

export const modVersionDeleteHandler: WSMessageHandler = {
	type: MessageType.MOD_VERSION_DELETE,
	async handle(data: Reply & { version_id: string; removed?: boolean }) {
		if (settled(MessageType.MOD_VERSION_DELETE, data)) return;
		const state = getModsState();
		state.loadLibrary();
		state.planLoaded();
	}
};

export const modTargetScanHandler: WSMessageHandler = {
	type: MessageType.MOD_TARGET_SCAN,
	async handle(data: Reply & Partial<ScanReport> & { target_id: string }) {
		const state = getModsState();
		const full = state.completeScan(data.target_id);
		if (data.canceled) return;
		const error = errorOf(data);
		if (error) {
			state.refuseScan(data.target_id);
			return refuse(
				MessageType.MOD_TARGET_SCAN,
				error,
				data.target_id,
				toastsRefusal(MessageType.MOD_TARGET_SCAN, data.target_id)
			);
		}
		state.clearLastError(MessageType.MOD_TARGET_SCAN, data.target_id);
		state.recordScan(data.target_id, data, full);
	}
};

export const modAdoptHandler: WSMessageHandler = {
	type: MessageType.MOD_ADOPT,
	async handle(
		data: Reply & {
			target_id: string;
			candidate_name: string;
			mod_id?: string;
		}
	) {
		const state = getModsState();
		state.finishBusy('adopting', data.target_id);
		if (settled(MessageType.MOD_ADOPT, data, data.target_id)) return;
		getToastState().add(
			m.mods_panel_adopted({ name: data.candidate_name }),
			m.mods_panel_title(),
			'success'
		);
		state.loadLibrary();
		state.loadProfiles(data.target_id);
		state.refreshScan(data.target_id);
		state.plan(data.target_id);
	}
};

export const modBackupListHandler: WSMessageHandler = {
	type: MessageType.MOD_BACKUP_LIST,
	async handle(data: Reply & { target_id: string; sets?: BackupSet[] }) {
		if (settled(MessageType.MOD_BACKUP_LIST, data, data.target_id)) return;
		const state = getModsState();
		state.clearLastError(MessageType.MOD_BACKUP_RESTORE, data.target_id);
		state.clearLastError(MessageType.MOD_BACKUP_DELETE, data.target_id);
		state.backups = { ...state.backups, [data.target_id]: data.sets ?? [] };
	}
};

export const modBackupRestoreHandler: WSMessageHandler = {
	type: MessageType.MOD_BACKUP_RESTORE,
	async handle(
		data: Reply & { target_id: string; set: string; restored?: string[]; skipped?: RestoreSkip[] }
	) {
		const state = getModsState();
		state.finishBusy('restoring', data.target_id);
		if (settled(MessageType.MOD_BACKUP_RESTORE, data, data.target_id)) return;
		state.lastRestore = {
			...state.lastRestore,
			[data.target_id]: {
				target_id: data.target_id,
				set: data.set,
				restored: data.restored ?? [],
				skipped: data.skipped ?? []
			}
		};
		state.loadBackups(data.target_id);
		state.rescan(data.target_id);
		state.plan(data.target_id);
	}
};

export const modBackupDeleteHandler: WSMessageHandler = {
	type: MessageType.MOD_BACKUP_DELETE,
	async handle(data: Reply & { target_id: string; set: string; removed?: boolean }) {
		const state = getModsState();
		state.finishBusy('deleting', data.target_id);
		if (settled(MessageType.MOD_BACKUP_DELETE, data, data.target_id)) return;
		state.loadBackups(data.target_id);
	}
};

export const modProgressHandler: WSMessageHandler = {
	type: MessageType.MOD_PROGRESS,
	async handle(data: ModProgress) {
		const state = getModsState();
		if (data.stage === 'done') return state.clearProgress(data.request_id);
		state.progress = { ...state.progress, [data.request_id]: data };
	}
};

/** The server answers this with the legacy string refusal; `errorOf` gives it a code. */
export const listLocalSavesHandler: WSMessageHandler = {
	type: MessageType.LIST_LOCAL_SAVES,
	async handle(data: Reply & { saves?: LocalSave[] }) {
		const state = getModsState();
		state.loadingSaves = false;
		if (settled(MessageType.LIST_LOCAL_SAVES, data, undefined, { toast: false })) return;
		state.localSaves = data.saves ?? [];
	}
};

export const worldProfileSetHandler: WSMessageHandler = {
	type: MessageType.WORLD_PROFILE_SET,
	async handle(
		data: Reply & {
			world_key?: string;
			world_name?: string;
			profile_id?: string | null;
			linked?: boolean;
		}
	) {
		const state = getModsState();
		state.finishBusy('linkingWorld', data.world_key);
		if (data.canceled) return;
		const error = errorOf(data);
		if (error) {
			state.recordRefusal(MessageType.WORLD_PROFILE_SET, {
				...error,
				world_key: data.world_key,
				world_name: data.world_name
			});
			return;
		}
		state.clearLastError(MessageType.WORLD_PROFILE_SET);
		if (data.world_key) state.recordWorldLink(data.world_key, data.profile_id ?? null);
		for (const targetId of Object.keys(state.profiles)) state.loadProfiles(targetId);
		state.loadLocalSaves();
	}
};

/** Refused after the linked profile was activated and applied, which a failed launch keeps. */
const launchRefusedAfterApply = new Set([
	'apply_failed',
	'layout_error',
	'unsupported_platform',
	'launch_unavailable',
	'launch_failed',
	'db'
]);

export const gameLaunchHandler: WSMessageHandler = {
	type: MessageType.GAME_LAUNCH,
	async handle(data: Reply & Partial<Omit<LaunchReply, 'target_id'>> & { target_id: string }) {
		const state = getModsState();
		state.finishBusy('launching', data.target_id);
		if (settled(MessageType.GAME_LAUNCH, data, data.target_id)) {
			const error = errorOf(data);
			if (!error || !launchRefusedAfterApply.has(error.code)) return;
			const apply = error.apply as ApplyResult | undefined;
			if (apply?.request_id !== undefined) recordApply(apply);
			state.loadProfiles(data.target_id);
			state.plan(data.target_id);
			return;
		}
		state.lastLaunch = { ...state.lastLaunch, [data.target_id]: data as LaunchReply };
		if (data.apply && data.apply.request_id !== undefined) recordApply(data.apply);
		state.pending = { ...state.pending, [data.target_id]: false };
		if (data.activated) state.loadProfiles(data.target_id);
		state.plan(data.target_id);
	}
};

export const modUploadBeginHandler: WSMessageHandler = {
	type: MessageType.MOD_UPLOAD_BEGIN,
	async handle(data: Reply & { upload_id?: string; chunk_size?: number; name?: string }) {
		const state = getModsState();
		const error = errorOf(data);
		if (error) return state.uploadRefused(error, data.upload_id);
		if (data.upload_id && data.chunk_size) state.uploadBegun(data.upload_id, data.chunk_size);
	}
};

export const modUploadChunkHandler: WSMessageHandler = {
	type: MessageType.MOD_UPLOAD_CHUNK,
	async handle(data: Reply & { upload_id?: string; seq?: number; received?: number }) {
		const state = getModsState();
		const error = errorOf(data);
		if (error) return state.uploadRefused(error, data.upload_id);
		if (data.upload_id && typeof data.seq === 'number' && typeof data.received === 'number') {
			state.uploadChunkAccepted(data.upload_id, data.seq, data.received);
		}
	}
};

export const modUploadEndHandler: WSMessageHandler = {
	type: MessageType.MOD_UPLOAD_END,
	async handle(data: Reply & { upload_id?: string; path?: string }) {
		const state = getModsState();
		const error = errorOf(data);
		if (error) return state.uploadRefused(error, data.upload_id);
		if (data.upload_id && data.path) state.uploadEnded(data.upload_id, data.path);
	}
};

export const profileExportHandler: WSMessageHandler = {
	type: MessageType.PROFILE_EXPORT,
	async handle(data: Reply & Partial<Omit<ExportReply, 'target_id'>> & { target_id: string }) {
		const state = getModsState();
		state.finishBusy('exporting', data.target_id);
		if (settled(MessageType.PROFILE_EXPORT, data, data.target_id)) return;
		state.lastExport = { ...state.lastExport, [data.target_id]: data as ExportReply };
	}
};

export const profileImportHandler: WSMessageHandler = {
	type: MessageType.PROFILE_IMPORT,
	async handle(data: Reply & Partial<Omit<ImportReply, 'target_id'>> & { target_id: string }) {
		const state = getModsState();
		state.finishBusy('importing', data.target_id);
		if (settled(MessageType.PROFILE_IMPORT, data, data.target_id)) {
			if (errorOf(data)?.profile_id) state.loadProfiles(data.target_id);
			return;
		}
		state.lastImport = { ...state.lastImport, [data.target_id]: data as ImportReply };
		if (data.profile) {
			state.upsertProfile(data.profile);
			state.viewProfile(data.target_id, data.profile.id);
		}
		state.loadProfiles(data.target_id);
		if ((data.installed ?? []).length > 0) state.loadLibrary();
	}
};

export const frameworkStatusHandler: WSMessageHandler = {
	type: MessageType.FRAMEWORK_STATUS,
	async handle(data: Reply & Partial<FrameworkStatus>) {
		if (settled(MessageType.FRAMEWORK_STATUS, data, data.target_id)) return;
		const state = getModsState();
		if (data.target_id)
			state.frameworks = { ...state.frameworks, [data.target_id]: data as FrameworkStatus };
	}
};

function afterFrameworkChange(targetId: string, reply: SelectionReply): void {
	const state = getModsState();
	applySelection(targetId, reply);
	state.loadFrameworks(targetId);
	state.loadLibrary();
	state.loadTargets();
	state.plan(targetId);
	if (state.conflicts[targetId]) state.loadConflicts(targetId);
}

export const frameworkInstallHandler: WSMessageHandler = {
	type: MessageType.FRAMEWORK_INSTALL,
	async handle(data: Reply & Partial<FrameworkInstallReply>) {
		const state = getModsState();
		state.finishBusy('frameworkBusy', data.target_id);
		if (data.canceled) return;
		const error = errorOf(data);
		if (error) {
			state.recordRefusal(
				MessageType.FRAMEWORK_INSTALL,
				{ ...error, key: data.key },
				data.target_id
			);
			return;
		}
		state.clearLastError(MessageType.FRAMEWORK_INSTALL, data.target_id);
		if (data.target_id) afterFrameworkChange(data.target_id, data);
	}
};

export const frameworkRemoveHandler: WSMessageHandler = {
	type: MessageType.FRAMEWORK_REMOVE,
	async handle(data: Reply & Partial<FrameworkRemoveReply>) {
		const state = getModsState();
		state.finishBusy('frameworkBusy', data.target_id);
		if (data.canceled) return;
		const error = errorOf(data);
		if (error) {
			state.recordRefusal(
				MessageType.FRAMEWORK_REMOVE,
				{ ...error, key: data.key },
				data.target_id
			);
			return;
		}
		state.clearLastError(MessageType.FRAMEWORK_REMOVE, data.target_id);
		if (data.target_id) afterFrameworkChange(data.target_id, data);
	}
};

export const frameworkHazardRemoveHandler: WSMessageHandler = {
	type: MessageType.FRAMEWORK_HAZARD_REMOVE,
	async handle(data: Reply & Partial<HazardRemoveReply>) {
		const state = getModsState();
		state.finishBusy('frameworkBusy', data.target_id);
		if (settled(MessageType.FRAMEWORK_HAZARD_REMOVE, data, data.target_id)) return;
		if (!data.target_id) return;
		state.lastHazardRemove = {
			...state.lastHazardRemove,
			[data.target_id]: data as HazardRemoveReply
		};
		state.loadTargets();
		state.loadFrameworks(data.target_id);
	}
};

export const modConflictsHandler: WSMessageHandler = {
	type: MessageType.MOD_CONFLICTS,
	async handle(data: Reply & Partial<ConflictReport>) {
		const state = getModsState();
		state.finishBusy('checkingConflicts', data.target_id);
		if (settled(MessageType.MOD_CONFLICTS, data, data.target_id)) return;
		if (data.target_id) {
			state.conflicts = { ...state.conflicts, [data.target_id]: data as ConflictReport };
		}
	}
};

export const modIostoreConvertHandler: WSMessageHandler = {
	type: MessageType.MOD_IOSTORE_CONVERT,
	async handle(data: Reply & Partial<IostoreConvertReply>) {
		const state = getModsState();
		state.finishBusy('converting', data.target_id);
		if (settled(MessageType.MOD_IOSTORE_CONVERT, data, data.target_id)) return;
		if (!data.target_id) return;
		applySelection(data.target_id, data);
		state.loadLibrary();
		state.loadProfiles(data.target_id);
		state.plan(data.target_id);
		if (state.conflicts[data.target_id]) state.loadConflicts(data.target_id);
	}
};

export const modVerificationHandler: WSMessageHandler = {
	type: MessageType.MOD_VERIFICATION,
	async handle(data: TargetVerification) {
		const state = getModsState();
		state.verification = { ...state.verification, [data.target_id]: data };
	}
};

export const modVerificationGetHandler: WSMessageHandler = {
	type: MessageType.MOD_VERIFICATION_GET,
	async handle(data: Reply & { target_id: string; verification?: TargetVerification | null }) {
		if (settled(MessageType.MOD_VERIFICATION_GET, data, data.target_id)) return;
		const state = getModsState();
		const rest = Object.fromEntries(
			Object.entries(state.verification).filter(([targetId]) => targetId !== data.target_id)
		);
		state.verification = data.verification
			? { ...rest, [data.target_id]: data.verification }
			: rest;
	}
};

export const modVerificationSubscribeHandler: WSMessageHandler = {
	type: MessageType.MOD_VERIFICATION_SUBSCRIBE,
	async handle(data: Reply) {
		settled(MessageType.MOD_VERIFICATION_SUBSCRIBE, data);
	}
};

export const modsHandlers = [
	modTargetListHandler,
	modTargetDetectHandler,
	modTargetAddHandler,
	modTargetRemoveHandler,
	modListHandler,
	profileListHandler,
	profileSetModHandler,
	profileCreateHandler,
	profileRenameHandler,
	profileDeleteHandler,
	profileActivateHandler,
	profileReorderHandler,
	profileSetOptionsHandler,
	profilePlanHandler,
	profileApplyHandler,
	modAnalyzeHandler,
	modInstallHandler,
	modRemoveHandler,
	modVersionSetCurrentHandler,
	modVersionDeleteHandler,
	modTargetScanHandler,
	modAdoptHandler,
	modBackupListHandler,
	modBackupRestoreHandler,
	modBackupDeleteHandler,
	modProgressHandler,
	listLocalSavesHandler,
	worldProfileSetHandler,
	gameLaunchHandler,
	modUploadBeginHandler,
	modUploadChunkHandler,
	modUploadEndHandler,
	profileExportHandler,
	profileImportHandler,
	frameworkStatusHandler,
	frameworkInstallHandler,
	frameworkRemoveHandler,
	frameworkHazardRemoveHandler,
	modConflictsHandler,
	modIostoreConvertHandler,
	modVerificationHandler,
	modVerificationGetHandler,
	modVerificationSubscribeHandler,
	profileRemoveModHandler,
	modReleaseProfilesHandler
];
