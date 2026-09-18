import { MAX_UPLOAD_BYTES, bytesToBase64, readChunk, sha256Hex } from '$lib/utils/modUpload';
import { send } from '$lib/utils/websocketUtils';
import type {
	AdoptionCandidate,
	AnalyzeReply,
	ApplyResult,
	BackupSet,
	ConflictReport,
	DetectedInstall,
	ExportReply,
	FrameworkKey,
	FrameworkStatus,
	HazardRemoveReply,
	ImportReply,
	InstallRecord,
	LaunchReply,
	LibraryMod,
	LocalSave,
	ModError,
	ModInUseHolder,
	ModProfile,
	ModProgress,
	ModTarget,
	ModVerification,
	PendingDecisions,
	ProfileOptions,
	RecordedModError,
	RefusalSubject,
	ReleaseProfilesReply,
	ReorderKind,
	RestoreReply,
	ScanReport,
	TargetPlan,
	TargetVerification,
	UploadPurpose,
	UploadStatus
} from '$types';
import { MessageType } from '$types';
import { untrack } from 'svelte';

export type {
	AdoptionCandidate,
	ApplyResult,
	InstallRecord,
	LibraryMod,
	ModError,
	ModProfile,
	ModProgress,
	ModTarget,
	ModVersion,
	NeedsAttention,
	PendingDecisions,
	PlanOp,
	ProfileMod,
	RecordedModError
} from '$types';

export interface InstallOptions {
	acceptDefaults?: boolean;
	enable?: boolean;
	customName?: string;
}

export type BusyKind =
	| 'settingMod'
	| 'installing'
	| 'restoring'
	| 'adopting'
	| 'analyzing'
	| 'deleting'
	| 'managingProfile'
	| 'activating'
	| 'ordering'
	| 'launching'
	| 'linkingWorld'
	| 'exporting'
	| 'importing'
	| 'frameworkBusy'
	| 'checkingConflicts'
	| 'converting'
	| 'releasing';

/** Untargeted errors (a library-wide refusal) are keyed under `_`. */
export function errorKey(code: string, targetId?: string): string {
	return `${targetId ?? '_'}:${code}`;
}

const subjectFields = ['mod_id', 'version_id', 'candidate_name', 'source', 'root'] as const;
const candidateFields = ['candidate_name', 'source', 'root'] as const;

function subjectKey(subject: RefusalSubject | undefined): string {
	if (!subject) return '';
	return subjectFields
		.map((field) => (subject[field] === undefined ? '' : `#${field}=${subject[field]}`))
		.join('');
}

/** Mod ids decide when both name one; otherwise version ids; otherwise candidates; otherwise they match. */
function sameSubject(stored: RefusalSubject, reply: RefusalSubject): boolean {
	if (stored.mod_id !== undefined && reply.mod_id !== undefined)
		return stored.mod_id === reply.mod_id;
	if (stored.version_id !== undefined && reply.version_id !== undefined)
		return stored.version_id === reply.version_id;
	if (stored.candidate_name !== undefined && reply.candidate_name !== undefined)
		return candidateFields.every((field) => stored[field] === reply[field]);
	return true;
}

function namesSubject(stored: RefusalSubject | undefined, wanted: RefusalSubject): boolean {
	return (Object.keys(wanted) as (keyof RefusalSubject)[]).every(
		(field) => wanted[field] === undefined || stored?.[field] === wanted[field]
	);
}

function namesCandidate(stored: RefusalSubject | undefined, candidate: AdoptionCandidate): boolean {
	return (
		stored?.candidate_name === candidate.name &&
		stored.source === candidate.source &&
		stored.root === candidate.root
	);
}

function without<T>(record: Record<string, T>, key: string): Record<string, T> {
	return Object.fromEntries(Object.entries(record).filter(([entry]) => entry !== key));
}

export class ModsState {
	targets = $state<ModTarget[]>([]);
	detected = $state<DetectedInstall[]>([]);
	profiles = $state<Record<string, ModProfile[]>>({});
	mods = $state<LibraryMod[]>([]);
	plans = $state<Record<string, TargetPlan>>({});
	scans = $state<Record<string, ScanReport>>({});
	/** Whether the latest scan reply for a target was a full report; a refused scan counts as not full. */
	scanReplyFull = $state<Record<string, boolean>>({});
	/** `Date.now()` when a target's last full report arrived; the server never stamps a scan. */
	lastFullScanAt = $state<Record<string, number>>({});
	backups = $state<Record<string, BackupSet[]>>({});
	analysis = $state<Record<string, AnalyzeReply>>({});
	lastApply = $state<Record<string, ApplyResult>>({});
	lastRestore = $state<Record<string, RestoreReply>>({});
	lastInstall = $state<Record<string, InstallRecord>>({});
	installSeq = $state<Record<string, number>>({});
	progress = $state<Record<string, ModProgress>>({});
	/** Keyed per type, target and the subject the refusal named, so two rows' refusals coexist. */
	lastError = $state<Record<string, RecordedModError>>({});
	applying = $state<Record<string, boolean>>({});
	pending = $state<Record<string, boolean>>({});
	installing = $state<Record<string, boolean>>({});
	analyzing = $state<Record<string, boolean>>({});
	scanning = $state<Record<string, boolean>>({});
	restoring = $state<Record<string, boolean>>({});
	deleting = $state<Record<string, boolean>>({});
	settingMod = $state<Record<string, boolean>>({});
	managingProfile = $state<Record<string, boolean>>({});
	activating = $state<Record<string, boolean>>({});
	ordering = $state<Record<string, boolean>>({});
	localSaves = $state<LocalSave[] | undefined>(undefined);
	loadingSaves = $state(false);
	launching = $state<Record<string, boolean>>({});
	linkingWorld = $state<Record<string, boolean>>({});
	lastLaunch = $state<Record<string, LaunchReply>>({});
	exporting = $state<Record<string, boolean>>({});
	importing = $state<Record<string, boolean>>({});
	lastExport = $state<Record<string, ExportReply>>({});
	lastImport = $state<Record<string, ImportReply>>({});
	frameworks = $state<Record<string, FrameworkStatus>>({});
	frameworkBusy = $state<Record<string, boolean>>({});
	lastHazardRemove = $state<Record<string, HazardRemoveReply>>({});
	conflicts = $state<Record<string, ConflictReport>>({});
	checkingConflicts = $state<Record<string, boolean>>({});
	converting = $state<Record<string, boolean>>({});
	releasing = $state<Record<string, boolean>>({});
	lastRelease = $state<Record<string, ReleaseProfilesReply>>({});
	verification = $state<Record<string, TargetVerification>>({});
	viewedProfileId = $state<Record<string, string>>({});
	needsDecisions = $state<Record<string, PendingDecisions>>({});
	candidates = $state<Record<string, AdoptionCandidate[]>>({});
	adopting = $state<Record<string, boolean>>({});
	scanWarnings = $state<Record<string, string[]>>({});
	/** Advanced by every `mod_target_add` reply; `targetId` is set only on success. */
	addReply = $state<{ seq: number; targetId: string | null }>({ seq: 0, targetId: null });
	detecting = $state(false);
	targetsLoaded = $state(false);
	resets = $state(0);
	uploadStatus = $state<UploadStatus | null>(null);
	#uploadFile: Blob | null = null;
	#uploadToken = 0;
	#chunkSize = 0;
	/** Begin replies still owed to uploads abandoned before theirs arrived; replies come in request order. */
	#staleBegins = 0;
	#lastScanFull = new Map<string, boolean>();
	#scansInFlight = new Map<string, boolean[]>();
	#installsInFlight = new Map<string, number[]>();
	#answeredInstall = new Map<string, number>();
	#inFlight = new Map<string, number>();
	#connected: boolean | undefined;
	#wasConnected = false;
	#transport: object | null | undefined;
	#verificationSubscribed = false;

	subscribedCandidates(targetId: string): AdoptionCandidate[] {
		return (this.candidates[targetId] ?? []).filter(
			(candidate) => candidate.source === 'steam_subscribed'
		);
	}

	activeProfile(targetId: string): ModProfile | undefined {
		return this.profiles[targetId]?.find((profile) => profile.is_active);
	}

	enabledOn(targetId: string, modId: string): boolean {
		return (
			this.activeProfile(targetId)?.mods.find((entry) => entry.mod_id === modId)?.enabled ?? false
		);
	}

	recordRefusal(
		type: string,
		error: ModError,
		targetId?: string,
		options: { subject?: RefusalSubject } = {}
	): void {
		const detailTarget = typeof error.target_id === 'string' ? error.target_id : undefined;
		const recorded: RecordedModError = { ...error, target_id: targetId ?? detailTarget };
		if (options.subject) recorded.subject = options.subject;
		else delete recorded.subject;
		const key = errorKey(type, targetId) + subjectKey(options.subject);
		this.lastError = { ...without(this.lastError, key), [key]: recorded };
	}

	#refusals(type: string, targetId?: string): [string, RecordedModError][] {
		const base = errorKey(type, targetId);
		return Object.entries(this.lastError).filter(
			([key]) => key === base || key.startsWith(`${base}#`)
		);
	}

	#forgetRefusals(keys: string[]): void {
		if (keys.length === 0) return;
		const forgotten = new Set(keys);
		this.lastError = Object.fromEntries(
			Object.entries(this.lastError).filter(([key]) => !forgotten.has(key))
		);
	}

	/** The latest refusal of the type for the target; with a subject, only one that named it. */
	lastErrorFor(
		type: string,
		targetId?: string,
		subject?: RefusalSubject
	): RecordedModError | undefined {
		return this.#refusals(type, targetId)
			.filter(([, stored]) => !subject || namesSubject(stored.subject, subject))
			.at(-1)?.[1];
	}

	lastErrorsFor(type: string, targetId?: string): RecordedModError[] {
		return this.#refusals(type, targetId).map(([, stored]) => stored);
	}

	/** A reply for one target or subject leaves another's refusal of the same type in place. */
	clearLastError(type: string, targetId?: string, subject?: RefusalSubject): void {
		const candidates =
			targetId === undefined
				? this.#refusals(type)
				: [...this.#refusals(type, targetId), ...this.#refusals(type)];
		const cleared = candidates.filter(([key, stored]) => {
			const otherTarget =
				targetId !== undefined &&
				key.startsWith('_:') &&
				stored.target_id !== undefined &&
				stored.target_id !== targetId;
			if (otherTarget) return false;
			return !(subject && stored.subject && !sameSubject(stored.subject, subject));
		});
		this.#forgetRefusals(cleared.map(([key]) => key));
	}

	/** Applies a saved selection at once, so a toggle never re-enables showing its old state. */
	recordSetMod(
		targetId: string,
		profileId: string,
		modId: string,
		enabled: boolean,
		modVersionId: string | null
	): void {
		const profiles = this.profiles[targetId];
		if (!profiles) return;
		this.profiles = {
			...this.profiles,
			[targetId]: profiles.map((profile) => {
				if (profile.id !== profileId) return profile;
				const existing = profile.mods.some((entry) => entry.mod_id === modId);
				const mods = existing
					? profile.mods.map((entry) =>
							entry.mod_id === modId ? { ...entry, enabled, mod_version_id: modVersionId } : entry
						)
					: [
							...profile.mods,
							{
								profile_id: profileId,
								mod_id: modId,
								mod_version_id: modVersionId,
								enabled,
								load_order: Math.max(-1, ...profile.mods.map((entry) => entry.load_order)) + 1
							}
						];
				return { ...profile, mods };
			})
		};
	}

	removeFromProfile(targetId: string, modId: string, profileId?: string): void {
		this.clearLastError(MessageType.PROFILE_REMOVE_MOD, targetId, { mod_id: modId });
		this.#startBusy('settingMod', targetId);
		send(
			MessageType.PROFILE_REMOVE_MOD,
			profileId === undefined
				? { target_id: targetId, mod_id: modId }
				: { target_id: targetId, profile_id: profileId, mod_id: modId }
		);
	}

	recordProfileRemove(targetId: string, profileId: string, modId: string): void {
		const profiles = this.profiles[targetId];
		if (!profiles) return;
		this.profiles = {
			...this.profiles,
			[targetId]: profiles.map((profile) =>
				profile.id === profileId
					? { ...profile, mods: profile.mods.filter((entry) => entry.mod_id !== modId) }
					: profile
			)
		};
	}

	releaseProfiles(modId: string): void {
		this.clearLastError(MessageType.MOD_RELEASE_PROFILES, undefined, { mod_id: modId });
		this.lastRelease = without(this.lastRelease, modId);
		this.#startBusy('releasing', modId);
		send(MessageType.MOD_RELEASE_PROFILES, { mod_id: modId });
	}

	/** Profile ids are unique across targets, so the released ids alone pick the holders to drop. */
	recordRelease(reply: ReleaseProfilesReply): void {
		this.lastRelease = { ...this.lastRelease, [reply.mod_id]: reply };
		const subject = { mod_id: reply.mod_id };
		const stored = this.lastErrorFor(MessageType.MOD_REMOVE, undefined, subject);
		if (stored?.code !== 'version_in_use') return;
		const released = new Set(reply.removed.map((entry) => entry.profile_id));
		const error: ModError = { ...stored };
		if (Array.isArray(stored.holders)) {
			error.holders = (stored.holders as ModInUseHolder[]).filter(
				(holder) => !released.has(holder.profile_id)
			);
		}
		if (Array.isArray(stored.profiles)) {
			error.profiles = (stored.profiles as unknown[]).filter(
				(id) => typeof id !== 'string' || !released.has(id)
			);
		}
		delete error.subject;
		delete error.target_id;
		this.recordRefusal(MessageType.MOD_REMOVE, error, undefined, {
			subject: stored.subject ?? subject
		});
	}

	/**
	 * A request sent before a drop is never answered, so a drop forgets in-flight bookkeeping. The
	 * socket transport queues a request sent during the gap and answers it after reconnecting; the
	 * remote transport rejects it, so a remote reconnect forgets in-flight bookkeeping too. Every
	 * reconnect reloads, because a request lost to the gap may have changed what the server holds.
	 */
	connectionChanged(connected: boolean, transport: 'socket' | 'remote' = 'socket'): void {
		untrack(() => {
			const dropped = this.#connected === true && !connected;
			const reconnected = this.#connected === false && connected && this.#wasConnected;
			this.#connected = connected;
			if (connected) this.#wasConnected = true;
			if (dropped || (reconnected && transport === 'remote')) this.#forgetInFlight();
			if (reconnected) this.resets += 1;
		});
	}

	/** The first transport seen is the one loaded over; any other serves another backend. */
	transportChanged(transport: object | null): void {
		const changed = this.#transport !== undefined && transport !== this.#transport;
		this.#transport = transport;
		if (changed) this.reset();
	}

	reset(): void {
		this.#forgetInFlight();
		this.#lastScanFull.clear();
		this.targetsLoaded = false;
		this.resets = untrack(() => this.resets) + 1;
		this.targets = [];
		this.detected = [];
		this.profiles = {};
		this.mods = [];
		this.plans = {};
		this.scans = {};
		this.scanReplyFull = {};
		this.lastFullScanAt = {};
		this.backups = {};
		this.analysis = {};
		this.lastApply = {};
		this.lastRestore = {};
		this.lastInstall = {};
		this.installSeq = {};
		this.lastError = {};
		this.pending = {};
		this.needsDecisions = {};
		this.candidates = {};
		this.scanWarnings = {};
		this.viewedProfileId = {};
		this.localSaves = undefined;
		this.lastLaunch = {};
		this.lastExport = {};
		this.lastImport = {};
		this.frameworks = {};
		this.frameworkBusy = {};
		this.lastHazardRemove = {};
		this.conflicts = {};
		this.checkingConflicts = {};
		this.converting = {};
		this.releasing = {};
		this.lastRelease = {};
		this.verification = {};
		this.clearUpload();
	}

	#forgetInFlight(): void {
		if (untrack(() => this.uploading)) this.#failUpload({ code: 'connection_lost', message: '' });
		this.#verificationSubscribed = false;
		this.#staleBegins = 0;
		this.detecting = false;
		this.answerAdd(null);
		this.adopting = {};
		this.applying = {};
		this.installing = {};
		this.analyzing = {};
		this.scanning = {};
		this.restoring = {};
		this.deleting = {};
		this.settingMod = {};
		this.managingProfile = {};
		this.activating = {};
		this.ordering = {};
		this.loadingSaves = false;
		this.launching = {};
		this.linkingWorld = {};
		this.exporting = {};
		this.importing = {};
		this.frameworkBusy = {};
		this.checkingConflicts = {};
		this.converting = {};
		this.releasing = {};
		this.progress = {};
		this.verification = Object.fromEntries(
			Object.entries(this.verification).map(([targetId, entry]) => [
				targetId,
				{ ...entry, live: false }
			])
		);
		this.#inFlight.clear();
		for (const [targetId, queue] of this.#scansInFlight) {
			const answered = untrack(() => this.scanReplyFull[targetId]) !== undefined;
			if (queue.length > 0 && !answered && !queue.includes(true)) {
				this.#lastScanFull.delete(targetId);
			}
		}
		this.#scansInFlight.clear();
		this.#installsInFlight.clear();
		this.#answeredInstall.clear();
	}

	#startBusy(kind: BusyKind, targetId: string): void {
		const key = `${kind}:${targetId}`;
		this.#inFlight.set(key, (this.#inFlight.get(key) ?? 0) + 1);
		this[kind] = { ...this[kind], [targetId]: true };
	}

	/** Without a target the reply cannot be matched, so every request of that kind is released. */
	finishBusy(kind: BusyKind, targetId: string | undefined): void {
		if (kind === 'installing') this.#answerInstall(targetId);
		if (targetId === undefined) {
			for (const key of [...this.#inFlight.keys()]) {
				if (key.startsWith(`${kind}:`)) this.#inFlight.delete(key);
			}
			this[kind] = {};
			return;
		}
		const key = `${kind}:${targetId}`;
		const remaining = Math.max(0, (this.#inFlight.get(key) ?? 0) - 1);
		if (remaining === 0) this.#inFlight.delete(key);
		else this.#inFlight.set(key, remaining);
		this[kind] = { ...this[kind], [targetId]: remaining > 0 };
	}

	/** Replies arrive in request order, so a target's reply answers its oldest unanswered install. */
	#answerInstall(targetId: string | undefined): void {
		if (targetId === undefined) {
			this.#installsInFlight.clear();
			this.#answeredInstall.clear();
			return;
		}
		const [seq = 0, ...rest] = this.#installsInFlight.get(targetId) ?? [];
		this.#installsInFlight.set(targetId, rest);
		this.#answeredInstall.set(targetId, seq);
	}

	loadTargets(): void {
		send(MessageType.MOD_TARGET_LIST);
	}

	detect(): void {
		this.detecting = true;
		this.detected = [];
		send(MessageType.MOD_TARGET_DETECT);
	}

	answerAdd(targetId: string | null): void {
		this.addReply = { seq: untrack(() => this.addReply.seq) + 1, targetId };
	}

	addTarget(rootPath: string): void {
		send(MessageType.MOD_TARGET_ADD, { root_path: rootPath });
	}

	removeTarget(targetId: string): void {
		send(MessageType.MOD_TARGET_REMOVE, { target_id: targetId });
	}

	/** Client target ids are deterministic, so a re-added install must inherit nothing. */
	forgetTarget(targetId: string): void {
		this.#lastScanFull.delete(targetId);
		this.#scansInFlight.delete(targetId);
		this.#installsInFlight.delete(targetId);
		this.#answeredInstall.delete(targetId);
		for (const key of [...this.#inFlight.keys()]) {
			if (key.endsWith(`:${targetId}`)) this.#inFlight.delete(key);
		}
		this.targets = this.targets.filter((target) => target.id !== targetId);
		this.profiles = without(this.profiles, targetId);
		this.plans = without(this.plans, targetId);
		this.scans = without(this.scans, targetId);
		this.scanReplyFull = without(this.scanReplyFull, targetId);
		this.lastFullScanAt = without(this.lastFullScanAt, targetId);
		this.backups = without(this.backups, targetId);
		this.analysis = without(this.analysis, targetId);
		this.lastApply = without(this.lastApply, targetId);
		this.lastRestore = without(this.lastRestore, targetId);
		this.lastInstall = without(this.lastInstall, targetId);
		this.installSeq = without(this.installSeq, targetId);
		this.pending = without(this.pending, targetId);
		this.needsDecisions = without(this.needsDecisions, targetId);
		this.applying = without(this.applying, targetId);
		this.installing = without(this.installing, targetId);
		this.analyzing = without(this.analyzing, targetId);
		this.scanning = without(this.scanning, targetId);
		this.restoring = without(this.restoring, targetId);
		this.deleting = without(this.deleting, targetId);
		this.settingMod = without(this.settingMod, targetId);
		this.adopting = without(this.adopting, targetId);
		this.viewedProfileId = without(this.viewedProfileId, targetId);
		this.managingProfile = without(this.managingProfile, targetId);
		this.activating = without(this.activating, targetId);
		this.ordering = without(this.ordering, targetId);
		this.launching = without(this.launching, targetId);
		this.lastLaunch = without(this.lastLaunch, targetId);
		this.exporting = without(this.exporting, targetId);
		this.importing = without(this.importing, targetId);
		this.lastExport = without(this.lastExport, targetId);
		this.lastImport = without(this.lastImport, targetId);
		this.frameworks = without(this.frameworks, targetId);
		this.frameworkBusy = without(this.frameworkBusy, targetId);
		this.lastHazardRemove = without(this.lastHazardRemove, targetId);
		this.conflicts = without(this.conflicts, targetId);
		this.checkingConflicts = without(this.checkingConflicts, targetId);
		this.converting = without(this.converting, targetId);
		this.verification = without(this.verification, targetId);
		this.clearCandidates(targetId);
		this.clearTargetProgress(targetId);
		this.lastError = Object.fromEntries(
			Object.entries(this.lastError).filter(
				([key, error]) => !key.startsWith(`${targetId}:`) && error.target_id !== targetId
			)
		);
	}

	loadLibrary(): void {
		send(MessageType.MOD_LIST);
	}

	loadProfiles(targetId: string): void {
		send(MessageType.PROFILE_LIST, { target_id: targetId });
	}

	setMod(targetId: string, modId: string, enabled: boolean, profileId?: string): void {
		this.#startBusy('settingMod', targetId);
		send(
			MessageType.PROFILE_SET_MOD,
			profileId === undefined
				? { target_id: targetId, mod_id: modId, enabled }
				: { target_id: targetId, profile_id: profileId, mod_id: modId, enabled }
		);
	}

	setModVersion(
		targetId: string,
		profileId: string,
		modId: string,
		enabled: boolean,
		versionId: string | null
	): void {
		this.#startBusy('settingMod', targetId);
		send(MessageType.PROFILE_SET_MOD, {
			target_id: targetId,
			profile_id: profileId,
			mod_id: modId,
			enabled,
			mod_version_id: versionId
		});
	}

	viewedProfile(targetId: string): ModProfile | undefined {
		const viewed = this.viewedProfileId[targetId];
		return (
			this.profiles[targetId]?.find((profile) => profile.id === viewed) ??
			this.activeProfile(targetId)
		);
	}

	viewProfile(targetId: string, profileId: string): void {
		this.viewedProfileId = { ...this.viewedProfileId, [targetId]: profileId };
	}

	enabledIn(targetId: string, profileId: string, modId: string): boolean {
		return (
			this.profiles[targetId]
				?.find((profile) => profile.id === profileId)
				?.mods.find((entry) => entry.mod_id === modId)?.enabled ?? false
		);
	}

	createProfile(targetId: string, name: string, copyFrom?: string): void {
		this.#startBusy('managingProfile', targetId);
		send(
			MessageType.PROFILE_CREATE,
			copyFrom ? { target_id: targetId, name, copy_from: copyFrom } : { target_id: targetId, name }
		);
	}

	renameProfile(targetId: string, profileId: string, name: string): void {
		this.#startBusy('managingProfile', targetId);
		send(MessageType.PROFILE_RENAME, { target_id: targetId, profile_id: profileId, name });
	}

	deleteProfile(targetId: string, profileId: string): void {
		this.#startBusy('managingProfile', targetId);
		send(MessageType.PROFILE_DELETE, { target_id: targetId, profile_id: profileId });
	}

	activateProfile(targetId: string, profileId: string): void {
		this.#startBusy('activating', targetId);
		send(MessageType.PROFILE_ACTIVATE, { target_id: targetId, profile_id: profileId });
	}

	reorderProfile(
		targetId: string,
		profileId: string,
		kind: ReorderKind,
		orderedModIds: string[]
	): void {
		this.recordOrder(targetId, profileId, orderedModIds);
		this.#startBusy('ordering', targetId);
		send(MessageType.PROFILE_REORDER, {
			target_id: targetId,
			profile_id: profileId,
			kind,
			ordered_mod_ids: orderedModIds
		});
	}

	setProfileOptions(targetId: string, profileId: string, options: ProfileOptions): void {
		this.#patchProfile(targetId, profileId, (profile) => ({ ...profile, ...options }));
		this.#startBusy('ordering', targetId);
		send(MessageType.PROFILE_SET_OPTIONS, {
			target_id: targetId,
			profile_id: profileId,
			...options
		});
	}

	/** The listed mods take the positions they already held, spread apart first when tied, as the server does. */
	recordOrder(targetId: string, profileId: string, orderedModIds: string[]): void {
		this.#patchProfile(targetId, profileId, (profile) => {
			const listed = new Set(orderedModIds);
			const held = profile.mods
				.filter((entry) => listed.has(entry.mod_id))
				.map((entry) => entry.load_order)
				.sort((a, b) => a - b);
			const slots =
				new Set(held).size === held.length ? held : held.map((_, index) => held[0] + index);
			const slotOf = new Map(orderedModIds.map((modId, index) => [modId, slots[index]]));
			return {
				...profile,
				mods: profile.mods.map((entry) => {
					const slot = slotOf.get(entry.mod_id);
					return slot === undefined ? entry : { ...entry, load_order: slot };
				})
			};
		});
	}

	upsertProfile(profile: ModProfile): void {
		const profiles = this.profiles[profile.target_id] ?? [];
		const exists = profiles.some((entry) => entry.id === profile.id);
		this.profiles = {
			...this.profiles,
			[profile.target_id]: exists
				? profiles.map((entry) => (entry.id === profile.id ? profile : entry))
				: [...profiles, profile]
		};
	}

	removeProfile(targetId: string, profileId: string): void {
		const profiles = this.profiles[targetId];
		if (profiles) {
			this.profiles = {
				...this.profiles,
				[targetId]: profiles.filter((profile) => profile.id !== profileId)
			};
		}
		if (this.viewedProfileId[targetId] === profileId) {
			this.viewedProfileId = without(this.viewedProfileId, targetId);
		}
	}

	markActive(targetId: string, profileId: string): void {
		const profiles = this.profiles[targetId];
		if (!profiles?.some((profile) => profile.id === profileId)) return;
		this.profiles = {
			...this.profiles,
			[targetId]: profiles.map((profile) => ({ ...profile, is_active: profile.id === profileId }))
		};
	}

	#patchProfile(
		targetId: string,
		profileId: string,
		patch: (profile: ModProfile) => ModProfile
	): void {
		const profiles = this.profiles[targetId];
		if (!profiles) return;
		this.profiles = {
			...this.profiles,
			[targetId]: profiles.map((profile) => (profile.id === profileId ? patch(profile) : profile))
		};
	}

	loadLocalSaves(): void {
		this.loadingSaves = true;
		send(MessageType.LIST_LOCAL_SAVES, { include_gamepass: true });
	}

	setWorldProfile(worldKey: string, worldName: string, profileId: string | null): void {
		this.#startBusy('linkingWorld', worldKey);
		send(MessageType.WORLD_PROFILE_SET, {
			world_key: worldKey,
			world_name: worldName,
			profile_id: profileId
		});
	}

	recordWorldLink(worldKey: string, profileId: string | null): void {
		const saves = this.localSaves;
		if (!saves) return;
		const profile =
			profileId === null
				? undefined
				: Object.values(this.profiles)
						.flat()
						.find((entry) => entry.id === profileId);
		const link =
			profileId === null
				? null
				: {
						profile_id: profileId,
						profile_name: profile?.name ?? profileId,
						target_id: profile?.target_id ?? ''
					};
		this.localSaves = saves.map((entry) =>
			entry.world_key === worldKey ? { ...entry, mod_profile: link } : entry
		);
	}

	launch(targetId: string, worldKey?: string): void {
		this.lastLaunch = without(this.lastLaunch, targetId);
		this.clearLastError(MessageType.GAME_LAUNCH, targetId);
		this.#startBusy('launching', targetId);
		send(
			MessageType.GAME_LAUNCH,
			worldKey === undefined
				? { target_id: targetId }
				: { target_id: targetId, world_key: worldKey }
		);
	}

	dismissLaunch(targetId: string): void {
		this.lastLaunch = without(this.lastLaunch, targetId);
	}

	exportProfile(targetId: string, profileId: string, includeArchives: boolean, path: string): void {
		this.lastExport = without(this.lastExport, targetId);
		this.clearLastError(MessageType.PROFILE_EXPORT, targetId);
		this.#startBusy('exporting', targetId);
		send(MessageType.PROFILE_EXPORT, {
			target_id: targetId,
			profile_id: profileId,
			include_archives: includeArchives,
			path
		});
	}

	importProfile(targetId: string, path: string, name?: string): void {
		this.lastImport = without(this.lastImport, targetId);
		this.clearLastError(MessageType.PROFILE_IMPORT, targetId);
		this.#startBusy('importing', targetId);
		const trimmed = name?.trim();
		send(
			MessageType.PROFILE_IMPORT,
			trimmed ? { target_id: targetId, path, name: trimmed } : { target_id: targetId, path }
		);
	}

	loadFrameworks(targetId: string, checkLatest = false): void {
		this.clearLastError(MessageType.FRAMEWORK_STATUS, targetId);
		send(
			MessageType.FRAMEWORK_STATUS,
			checkLatest ? { target_id: targetId, check_latest: true } : { target_id: targetId }
		);
	}

	installFramework(targetId: string, key: FrameworkKey, modVersionId?: string): void {
		this.clearLastError(MessageType.FRAMEWORK_INSTALL, targetId);
		this.clearLastError(MessageType.FRAMEWORK_REMOVE, targetId);
		this.#startBusy('frameworkBusy', targetId);
		send(
			MessageType.FRAMEWORK_INSTALL,
			modVersionId === undefined
				? { target_id: targetId, key }
				: { target_id: targetId, key, mod_version_id: modVersionId }
		);
	}

	removeFramework(targetId: string, key: FrameworkKey, force = false): void {
		this.clearLastError(MessageType.FRAMEWORK_REMOVE, targetId);
		this.clearLastError(MessageType.FRAMEWORK_INSTALL, targetId);
		this.#startBusy('frameworkBusy', targetId);
		send(
			MessageType.FRAMEWORK_REMOVE,
			force ? { target_id: targetId, key, force: true } : { target_id: targetId, key }
		);
	}

	removeHazard(targetId: string, hazard: string): void {
		this.clearLastError(MessageType.FRAMEWORK_HAZARD_REMOVE, targetId);
		this.lastHazardRemove = without(this.lastHazardRemove, targetId);
		this.#startBusy('frameworkBusy', targetId);
		send(MessageType.FRAMEWORK_HAZARD_REMOVE, { target_id: targetId, hazard });
	}

	loadConflicts(targetId: string): void {
		this.clearLastError(MessageType.MOD_CONFLICTS, targetId);
		this.#startBusy('checkingConflicts', targetId);
		send(MessageType.MOD_CONFLICTS, { target_id: targetId });
	}

	convertIostore(targetId: string, modId: string): void {
		this.clearLastError(MessageType.MOD_IOSTORE_CONVERT, targetId, { mod_id: modId });
		this.#startBusy('converting', targetId);
		send(MessageType.MOD_IOSTORE_CONVERT, { target_id: targetId, mod_id: modId });
	}

	subscribeVerification(): void {
		if (this.#verificationSubscribed) return;
		this.#verificationSubscribed = true;
		send(MessageType.MOD_VERIFICATION_SUBSCRIBE, {});
	}

	loadVerification(targetId: string): void {
		this.clearLastError(MessageType.MOD_VERIFICATION_GET, targetId);
		send(MessageType.MOD_VERIFICATION_GET, { target_id: targetId });
	}

	verificationFor(targetId: string, modId: string): ModVerification | undefined {
		return this.verification[targetId]?.status.find((entry) => entry.mod_id === modId);
	}

	get uploading(): boolean {
		const stage = this.uploadStatus?.stage;
		return stage === 'hashing' || stage === 'uploading' || stage === 'finishing';
	}

	/** Upload replies name no target, so only one upload runs at a time. */
	async startUpload(targetId: string, file: File, purpose: UploadPurpose): Promise<void> {
		if (this.uploading) return;
		const token = ++this.#uploadToken;
		const status: UploadStatus = {
			purpose,
			targetId,
			name: file.name,
			size: file.size,
			sent: 0,
			stage: 'hashing',
			uploadId: null,
			path: null,
			error: null
		};
		if (file.size > MAX_UPLOAD_BYTES) {
			this.uploadStatus = {
				...status,
				stage: 'failed',
				error: { code: 'upload_too_large', message: '', max_bytes: MAX_UPLOAD_BYTES }
			};
			return;
		}
		this.uploadStatus = status;
		let sha256: string;
		try {
			sha256 = await sha256Hex(file);
		} catch (error) {
			if (token === this.#uploadToken) {
				this.#failUpload({ code: 'read_failed', message: String(error) });
			}
			return;
		}
		if (token !== this.#uploadToken) return;
		this.#uploadFile = file;
		this.uploadStatus = { ...status, stage: 'uploading' };
		send(MessageType.MOD_UPLOAD_BEGIN, { name: file.name, size: file.size, sha256 });
	}

	uploadBegun(uploadId: string, chunkSize: number): void {
		if (this.#answersAbandonedBegin()) return;
		const status = this.uploadStatus;
		if (!status || status.stage !== 'uploading' || status.uploadId !== null) return;
		this.#chunkSize = chunkSize;
		this.uploadStatus = { ...status, uploadId };
		void this.#sendChunk(this.#uploadToken, uploadId, 0);
	}

	uploadChunkAccepted(uploadId: string, seq: number, received: number): void {
		const status = this.uploadStatus;
		if (!status || status.stage !== 'uploading' || status.uploadId !== uploadId) return;
		this.uploadStatus = { ...status, sent: received };
		void this.#sendChunk(this.#uploadToken, uploadId, seq + 1);
	}

	uploadEnded(uploadId: string, path: string): void {
		const status = this.uploadStatus;
		if (!status || status.stage !== 'finishing' || status.uploadId !== uploadId) return;
		this.#uploadFile = null;
		this.uploadStatus = { ...status, sent: status.size, stage: 'done', path };
	}

	/** A begin refusal carries no upload id, so it answers only an upload still waiting for one. */
	uploadRefused(error: ModError, uploadId: string | undefined): void {
		if (uploadId === undefined) {
			if (this.#answersAbandonedBegin() || !this.#beginOutstanding()) return;
			return this.#failUpload(error, true);
		}
		const status = this.uploadStatus;
		if (!status || !this.uploading || status.uploadId !== uploadId) return;
		this.#failUpload(error);
	}

	/** The server deletes an upload that stops receiving chunks, so cancelling only stops sending. */
	clearUpload(): void {
		if (this.#beginOutstanding()) this.#staleBegins += 1;
		this.#uploadToken += 1;
		this.#uploadFile = null;
		this.uploadStatus = null;
	}

	#beginOutstanding(): boolean {
		const status = untrack(() => this.uploadStatus);
		return status?.stage === 'uploading' && status.uploadId === null;
	}

	#answersAbandonedBegin(): boolean {
		if (this.#staleBegins === 0) return false;
		this.#staleBegins -= 1;
		return true;
	}

	#failUpload(error: ModError, beginAnswered = false): void {
		if (!beginAnswered && this.#beginOutstanding()) this.#staleBegins += 1;
		this.#uploadToken += 1;
		this.#uploadFile = null;
		if (this.uploadStatus) this.uploadStatus = { ...this.uploadStatus, stage: 'failed', error };
	}

	async #sendChunk(token: number, uploadId: string, seq: number): Promise<void> {
		const file = this.#uploadFile;
		if (!file || token !== this.#uploadToken) return;
		if (seq * this.#chunkSize >= file.size) {
			const status = this.uploadStatus;
			if (status) this.uploadStatus = { ...status, stage: 'finishing' };
			send(MessageType.MOD_UPLOAD_END, { upload_id: uploadId });
			return;
		}
		let bytes: Uint8Array;
		try {
			bytes = await readChunk(file, seq, this.#chunkSize);
		} catch (error) {
			if (token === this.#uploadToken) {
				this.#failUpload({ code: 'read_failed', message: String(error) });
			}
			return;
		}
		if (token !== this.#uploadToken || this.uploadStatus?.uploadId !== uploadId) return;
		send(MessageType.MOD_UPLOAD_CHUNK, {
			upload_id: uploadId,
			seq,
			data_b64: bytesToBase64(bytes)
		});
	}

	removeMod(modId: string): void {
		this.lastRelease = without(this.lastRelease, modId);
		send(MessageType.MOD_REMOVE, { mod_id: modId });
	}

	setCurrentVersion(modId: string, versionId: string): void {
		send(MessageType.MOD_VERSION_SET_CURRENT, { mod_id: modId, version_id: versionId });
	}

	deleteVersion(versionId: string): void {
		send(MessageType.MOD_VERSION_DELETE, { version_id: versionId });
	}

	plan(targetId: string): void {
		send(MessageType.PROFILE_PLAN, { target_id: targetId });
	}

	/** A refused plan is dropped, so its refusal is what keeps the target planned. */
	planLoaded(): void {
		const suffix = `:${MessageType.PROFILE_PLAN}`;
		const refused = Object.keys(this.lastError)
			.map((key) => key.split('#')[0])
			.filter((key) => key.endsWith(suffix))
			.map((key) => key.slice(0, -suffix.length))
			.filter((targetId) => targetId !== '_');
		for (const targetId of new Set([...Object.keys(this.plans), ...refused])) this.plan(targetId);
	}

	forgetPlan(targetId: string): void {
		this.plans = without(this.plans, targetId);
	}

	apply(targetId: string, replaceOccupants?: string[]): void {
		this.applying = { ...this.applying, [targetId]: true };
		send(
			MessageType.PROFILE_APPLY,
			replaceOccupants
				? { target_id: targetId, replace_occupants: replaceOccupants }
				: { target_id: targetId }
		);
	}

	analyze(targetId: string, path: string): void {
		this.analysis = without(this.analysis, targetId);
		this.#startBusy('analyzing', targetId);
		send(MessageType.MOD_ANALYZE, { target_id: targetId, path });
	}

	install(targetId: string, path: string, options: boolean | InstallOptions = false): void {
		const {
			acceptDefaults = false,
			enable,
			customName
		} = typeof options === 'boolean' ? { acceptDefaults: options } : options;
		this.needsDecisions = without(this.needsDecisions, targetId);
		this.lastInstall = without(this.lastInstall, targetId);
		const seq = (untrack(() => this.installSeq[targetId]) ?? 0) + 1;
		this.installSeq = { ...this.installSeq, [targetId]: seq };
		this.#installsInFlight.set(targetId, [...(this.#installsInFlight.get(targetId) ?? []), seq]);
		this.#startBusy('installing', targetId);
		const payload: Record<string, unknown> = {
			target_id: targetId,
			path,
			accept_defaults: acceptDefaults
		};
		if (enable !== undefined) payload.enable = enable;
		if (customName?.trim()) payload.custom_name = customName.trim();
		send(MessageType.MOD_INSTALL, payload);
	}

	/** Stamped with the install `finishBusy` released for this reply, or 0 when none was unanswered. */
	recordInstall(targetId: string, reply: Omit<InstallRecord, 'seq'>): void {
		this.lastInstall = {
			...this.lastInstall,
			[targetId]: { seq: this.#answeredInstall.get(targetId) ?? 0, ...reply }
		};
	}

	/** Once a target has had a full scan, a candidates-only request does not change what rescans repeat. */
	scan(targetId: string, options?: { full?: boolean }): void {
		const full = options?.full === true;
		this.#lastScanFull.set(targetId, full || this.#lastScanFull.get(targetId) === true);
		this.#scansInFlight.set(targetId, [...(this.#scansInFlight.get(targetId) ?? []), full]);
		if (full) this.scanning = { ...this.scanning, [targetId]: true };
		send(MessageType.MOD_TARGET_SCAN, { target_id: targetId, candidates_only: !full });
	}

	scanOnce(targetId: string): void {
		if (!this.#lastScanFull.has(targetId)) this.scan(targetId);
	}

	rescan(targetId: string): void {
		const full = this.#lastScanFull.get(targetId);
		if (full !== undefined) this.scan(targetId, { full });
	}

	rescanAll(): void {
		for (const [targetId, full] of [...this.#lastScanFull]) this.scan(targetId, { full });
	}

	refreshScan(targetId: string): void {
		this.scan(targetId, { full: this.#lastScanFull.get(targetId) === true });
	}

	/** Replies arrive in request order, so the oldest in-flight scan is the one answered. Returns whether it was full. */
	completeScan(targetId: string): boolean {
		const [full = false, ...rest] = this.#scansInFlight.get(targetId) ?? [];
		this.#scansInFlight.set(targetId, rest);
		this.scanning = { ...this.scanning, [targetId]: rest.includes(true) };
		return full;
	}

	/**
	 * Every reply refreshes candidates and warnings, and drops adopt refusals for candidates it no
	 * longer lists; only a full one replaces the report.
	 */
	recordScan(targetId: string, reply: Partial<ScanReport>, full: boolean): void {
		const candidates = reply.candidates ?? [];
		this.candidates = { ...this.candidates, [targetId]: candidates };
		this.scanWarnings = { ...this.scanWarnings, [targetId]: reply.warnings ?? [] };
		this.scanReplyFull = { ...this.scanReplyFull, [targetId]: full };
		this.#forgetRefusals(
			this.#refusals(MessageType.MOD_ADOPT, targetId)
				.filter(([, stored]) => !candidates.some((entry) => namesCandidate(stored.subject, entry)))
				.map(([key]) => key)
		);
		if (!full) return;
		this.lastFullScanAt = { ...this.lastFullScanAt, [targetId]: Date.now() };
		this.scans = {
			...this.scans,
			[targetId]: {
				target_id: targetId,
				files: reply.files ?? [],
				candidates,
				drifted: reply.drifted ?? [],
				missing: reply.missing ?? [],
				unreadable: reply.unreadable ?? [],
				warnings: reply.warnings ?? []
			}
		};
	}

	refuseScan(targetId: string): void {
		this.clearCandidates(targetId);
		this.scanReplyFull = { ...this.scanReplyFull, [targetId]: false };
	}

	clearCandidates(targetId: string): void {
		this.candidates = without(this.candidates, targetId);
		this.scanWarnings = without(this.scanWarnings, targetId);
	}

	adopt(targetId: string, candidate: AdoptionCandidate): void {
		this.#startBusy('adopting', targetId);
		send(MessageType.MOD_ADOPT, {
			target_id: targetId,
			candidate_name: candidate.name,
			source: candidate.source,
			root: candidate.root
		});
	}

	loadBackups(targetId: string): void {
		send(MessageType.MOD_BACKUP_LIST, { target_id: targetId });
	}

	restoreBackup(targetId: string, set: string, paths?: string[]): void {
		this.#startBusy('restoring', targetId);
		this.lastRestore = without(this.lastRestore, targetId);
		send(
			MessageType.MOD_BACKUP_RESTORE,
			paths ? { target_id: targetId, set, paths } : { target_id: targetId, set }
		);
	}

	deleteBackup(targetId: string, set: string): void {
		this.#startBusy('deleting', targetId);
		send(MessageType.MOD_BACKUP_DELETE, { target_id: targetId, set });
	}

	recordApply(result: ApplyResult): void {
		this.lastApply = { ...this.lastApply, [result.target_id]: result };
		this.progress = without(this.progress, result.request_id);
		this.applying = { ...this.applying, [result.target_id]: false };
	}

	clearProgress(requestId: string): void {
		this.progress = without(this.progress, requestId);
	}

	clearTargetProgress(targetId: string): void {
		this.progress = Object.fromEntries(
			Object.entries(this.progress).filter(([, entry]) => entry.target_id !== targetId)
		);
	}

	progressFor(targetId: string): ModProgress | undefined {
		return Object.values(this.progress).find((entry) => entry.target_id === targetId);
	}
}

let modsStateInstance: ModsState | undefined;

export function getModsState(): ModsState {
	if (!modsStateInstance) {
		modsStateInstance = new ModsState();
	}
	return modsStateInstance;
}
