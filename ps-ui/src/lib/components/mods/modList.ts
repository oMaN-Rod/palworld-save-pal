import * as m from '$i18n/messages';
import type {
	LibraryMod,
	ModInUseDeployed,
	ModInUseFramework,
	ModInUseHolder,
	ModTarget,
	ModVersion,
	RecordedModError
} from '$types';

export type ModSort = 'name' | 'type' | 'size';

export type RefusedKind = 'workshop' | 'nativedll';

const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];

export function formatSize(bytes: number | null): string {
	if (bytes === null) return '—';
	let value = bytes;
	let unit = 0;
	while (unit < units.length - 1 && Number(value.toFixed(1)) >= 1024) {
		value /= 1024;
		unit += 1;
	}
	return unit === 0 ? `${bytes} B` : `${value.toFixed(1)} ${units[unit]}`;
}

export function totalSize(mod: LibraryMod): number | null {
	const known = mod.versions.flatMap((version) =>
		version.size_bytes === null ? [] : [version.size_bytes]
	);
	return known.length === 0 ? null : known.reduce((sum, size) => sum + size, 0);
}

export function displayName(mod: LibraryMod): string {
	return mod.custom_name ?? mod.name;
}

/** Up to two letters standing in for a mod that has no thumbnail. */
export function initials(name: string): string {
	const words = name.split(/[^\p{L}\p{N}]+/u).filter(Boolean);
	return words.length === 0
		? '?'
		: words
				.slice(0, 2)
				.map((word) => word[0].toUpperCase())
				.join('');
}

export function versionsNewestFirst(mod: LibraryMod): ModVersion[] {
	return [...mod.versions].sort((a, b) => b.installed_at.localeCompare(a.installed_at));
}

export function formatDate(iso: string): string {
	const date = new Date(iso);
	return Number.isNaN(date.getTime()) ? iso : date.toLocaleDateString();
}

export function currentVersion(mod: LibraryMod): ModVersion | undefined {
	return mod.versions.find((version) => version.id === mod.current_version_id);
}

export function isSubscribed(mod: LibraryMod): boolean {
	if (mod.source_kind !== 'workshop') return false;
	try {
		const workshopId = (JSON.parse(mod.source_ref) as { workshop_id?: unknown })?.workshop_id;
		return typeof workshopId === 'string' && /^\d+$/.test(workshopId);
	} catch {
		return false;
	}
}

export function isClientOnlyOn(target: ModTarget, mod: LibraryMod): boolean {
	return (
		target.kind === 'server' && currentVersion(mod)?.manifest?.source?.server_capable === false
	);
}

/**
 * The route kind the target's layout has no folder for, judged on the pinned version or else the
 * current one. A missing version or unreadable manifest is allowed, as the server allows it.
 */
export function refusedKind(
	target: ModTarget,
	mod: LibraryMod,
	pinnedVersionId?: string | null
): RefusedKind | undefined {
	const version = pinnedVersionId
		? mod.versions.find((entry) => entry.id === pinnedVersionId)
		: currentVersion(mod);
	const manifest = version?.manifest;
	const layout = target.layout;
	if (!manifest || !layout) return undefined;
	const kinds = new Set<string>(manifest.routes.map((route) => route.kind));
	if (manifest.mod_type === 'workshop') kinds.add('workshop');
	if (kinds.has('workshop') && layout.workshop_local_dir === null) return 'workshop';
	if (kinds.has('nativedll') && layout.nativemods_dir === null) return 'nativedll';
	return undefined;
}

export interface UsageLabels {
	target(id: string): string;
	profile(id: string): string;
}

function idsOf(value: unknown): string[] {
	return Array.isArray(value) ? value.filter((id): id is string => typeof id === 'string') : [];
}

export function inUseProfileIds(error: RecordedModError): string[] {
	return idsOf(error.profiles);
}

/** Every reason a `version_in_use` refusal gives, in words; its message when it gives none. */
export function inUseLines(error: RecordedModError, labels: UsageLabels): string[] {
	const targets = idsOf(error.targets);
	const profiles = idsOf(error.profiles);
	const frameworks = idsOf(error.frameworks);
	const openApplies = idsOf(error.open_applies);
	const join = (ids: string[], label: (id: string) => string) =>
		ids.length > 0 ? ids.map(label).join(', ') : m.mods_list_in_use_none();
	const lines: string[] = [];
	if (error.is_current === true) lines.push(m.mods_list_in_use_current());
	if (targets.length > 0 || profiles.length > 0) {
		lines.push(
			m.mods_list_in_use({
				targets: join(targets, labels.target),
				profiles: join(profiles, labels.profile)
			})
		);
	}
	if (frameworks.length > 0) {
		lines.push(m.mods_list_in_use_frameworks({ targets: join(frameworks, labels.target) }));
	}
	if (openApplies.length > 0) {
		lines.push(m.mods_list_remove_busy({ target: join(openApplies, labels.target) }));
	}
	return lines.length > 0 ? lines : [error.message];
}

function isHolder(value: unknown): value is ModInUseHolder {
	const entry = value as ModInUseHolder | null;
	return (
		typeof entry === 'object' &&
		entry !== null &&
		typeof entry.target_id === 'string' &&
		typeof entry.profile_id === 'string' &&
		typeof entry.enabled === 'boolean'
	);
}

function isDeployed(value: unknown): value is ModInUseDeployed {
	const entry = value as ModInUseDeployed | null;
	return typeof entry === 'object' && entry !== null && typeof entry.target_id === 'string';
}

export function inUseHolders(error: RecordedModError): ModInUseHolder[] {
	return Array.isArray(error.holders) ? error.holders.filter(isHolder) : [];
}

export function inUseDeployed(error: RecordedModError): ModInUseDeployed[] {
	return Array.isArray(error.deployed) ? error.deployed.filter(isDeployed) : [];
}

export function inUseFrameworks(error: RecordedModError): ModInUseFramework[] {
	return Array.isArray(error.frameworks) ? error.frameworks.filter(isDeployed) : [];
}

export function releasable(error: RecordedModError): boolean {
	return inUseHolders(error).some((holder) => !holder.enabled);
}

/** A deployed target with an enabled holder cannot be cleared by applying, so it is not offered. */
export function applyOffers(error: RecordedModError): ModInUseDeployed[] {
	const holders = inUseHolders(error);
	return inUseDeployed(error).filter(
		(entry) => !holders.some((holder) => holder.target_id === entry.target_id && holder.enabled)
	);
}

function labelled(label: string, id: string, fallback: string | undefined): string {
	return label === id && fallback ? fallback : label;
}

export function holderText(holder: ModInUseHolder, labels: UsageLabels): string {
	const target = labelled(labels.target(holder.target_id), holder.target_id, holder.target_name);
	const profile = labelled(
		labels.profile(holder.profile_id),
		holder.profile_id,
		holder.profile_name
	);
	return holder.enabled
		? m.mods_in_use_holder_enabled({ target, profile })
		: m.mods_in_use_holder({ target, profile });
}

export function deployedText(entry: ModInUseDeployed, labels: UsageLabels): string {
	return labelled(labels.target(entry.target_id), entry.target_id, entry.target_name);
}

export function filterMods(mods: LibraryMod[], query: string): LibraryMod[] {
	const needle = query.trim().toLowerCase();
	if (!needle) return mods;
	return mods.filter(
		(mod) =>
			displayName(mod).toLowerCase().includes(needle) ||
			(mod.author ?? '').toLowerCase().includes(needle)
	);
}

export function sortMods(
	mods: LibraryMod[],
	sort: ModSort,
	typeLabel: (modType: string) => string
): LibraryMod[] {
	const byName = (a: LibraryMod, b: LibraryMod) =>
		displayName(a).localeCompare(displayName(b), undefined, { sensitivity: 'base' });
	const compare: Record<ModSort, (a: LibraryMod, b: LibraryMod) => number> = {
		name: byName,
		type: (a, b) => typeLabel(a.mod_type).localeCompare(typeLabel(b.mod_type)) || byName(a, b),
		size: (a, b) => (totalSize(b) ?? -1) - (totalSize(a) ?? -1) || byName(a, b)
	};
	return [...mods].sort(compare[sort]);
}
