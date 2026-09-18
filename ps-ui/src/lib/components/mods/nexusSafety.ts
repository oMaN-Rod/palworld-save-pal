import type { NexusFile } from '$types';

const SAFE_PROTOCOLS = new Set(['https:', 'http:']);

/** Nexus urls are untrusted: only an absolute http(s) one with no embedded credentials may reach `src` or `href`. */
export function safeHttpUrl(raw: string | null | undefined): string | null {
	if (!raw) return null;
	let parsed: URL;
	try {
		parsed = new URL(raw);
	} catch {
		return null;
	}
	if (!SAFE_PROTOCOLS.has(parsed.protocol)) return null;
	if (parsed.username || parsed.password) return null;
	return parsed.href;
}

export function modPageUrl(modId: number, fileId?: number): string {
	const base = `https://www.nexusmods.com/palworld/mods/${modId}`;
	return fileId === undefined ? base : `${base}?tab=files&file_id=${fileId}`;
}

export function nexusFileLabel(file: NexusFile): string {
	const name = file.name.trim();
	const version = file.version.trim();
	if (name && version) return `${name} ${version}`;
	if (name) return name;
	return `File ${file.file_id}`;
}
