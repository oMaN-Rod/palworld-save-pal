import type { FileRoute, InstallManifest, LibraryMod, ModTarget, ModVersion } from '$types';

export interface SessionMode {
	desktop: boolean;
	remote: boolean;
}

const GAMEPASS_PAK_DIRS = ['pal/content/paks/~mods/', 'pal/content/paks/logicmods/'];

function pathStemCi(relPath: string): string {
	const index = relPath.lastIndexOf('.');
	return (index === -1 ? relPath : relPath.slice(0, index)).toLowerCase();
}

function isPakRoute(route: FileRoute): boolean {
	if (route.kind === 'pak' || route.kind === 'logicmods') {
		return route.rel_path.toLowerCase().endsWith('.pak');
	}
	if (route.kind === 'passthrough') {
		const lower = route.rel_path.toLowerCase();
		return lower.endsWith('.pak') && GAMEPASS_PAK_DIRS.some((dir) => lower.startsWith(dir));
	}
	return false;
}

function hasIostoreSibling(manifest: InstallManifest, route: FileRoute): boolean {
	const base = pathStemCi(route.rel_path);
	const siblingWithExt = (extension: string) =>
		manifest.routes.some(
			(other) =>
				other.kind === route.kind &&
				other.rel_path.toLowerCase().endsWith(extension) &&
				pathStemCi(other.rel_path) === base
		);
	return siblingWithExt('.utoc') && siblingWithExt('.ucas');
}

/** Mirrors the server's `is_pak_route`/`iostore_sibling` rule: a legacy pak with no IoStore sibling. */
export function hasLegacyPak(manifest: InstallManifest): boolean {
	return manifest.routes.some((route) => isPakRoute(route) && !hasIostoreSibling(manifest, route));
}

/** A remote session is denied `game_launch`, and only the desktop app can start a local game. */
export function canLaunch(target: ModTarget, mode: SessionMode): boolean {
	return target.kind === 'client' && mode.desktop && !mode.remote;
}

/** `list_local_saves` needs a desktop server; a remote session always reaches one. */
export function canLinkWorlds(target: ModTarget, mode: SessionMode): boolean {
	return target.kind === 'client' && (mode.desktop || mode.remote);
}

/** `game_instance_set_target` is remote-denied. */
export function canBindInstances(mode: SessionMode): boolean {
	return !mode.remote;
}

/** `mod_iostore_convert` runs locally against a Game Pass client install and needs a legacy pak route. */
export function canConvertIostore(
	target: ModTarget,
	mode: SessionMode,
	mod: LibraryMod,
	version?: ModVersion
): boolean {
	if (!(mode.desktop && !mode.remote && target.kind === 'client' && target.platform === 'wingdk')) {
		return false;
	}
	const manifest = version?.manifest;
	if (manifest) return hasLegacyPak(manifest);
	return mod.mod_type === 'pak' || mod.mod_type === 'logicmods' || mod.mod_type === 'hybrid';
}
