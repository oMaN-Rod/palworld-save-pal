import { zipEntries } from '$lib/utils/folderUpload';
import { putRecent, getMostRecent, removeRecent, type RecentSave } from './recentSaves';
import { putBlob, getBlob, QuotaError } from './opfsBlobStore';
import { readSaveFolder, ensureReadWrite } from './fileSystemAccess';
import { setActiveDirectory } from './activeSave';

export async function recordSession(args: {
	zipBytes: Uint8Array;
	name: string;
	savedAt: number;
	handle?: FileSystemDirectoryHandle;
	writable?: boolean;
}): Promise<{ persisted: boolean; quota: boolean }> {
	const id = args.name || 'save';
	if (args.handle) {
		setActiveDirectory(args.handle, !!args.writable);
		const rec: RecentSave = {
			id,
			kind: 'handle',
			handle: args.handle,
			worldName: args.name,
			sizeBytes: args.zipBytes.length,
			savedAt: args.savedAt
		};
		// The save is already loaded; losing the recents entry only costs Resume.
		try {
			await putRecent(rec);
		} catch {
			return { persisted: false, quota: false };
		}
		return { persisted: true, quota: false };
	}
	const opfsPath = `${id}.zip`;
	try {
		await putBlob(opfsPath, args.zipBytes);
	} catch (e) {
		if (e instanceof QuotaError) return { persisted: false, quota: true };
		// OPFS unavailable (private window, embedded webview). Not fatal: the
		// save is loaded, only resume-after-reload is lost.
		return { persisted: false, quota: false };
	}
	const rec: RecentSave = {
		id,
		kind: 'opfs',
		opfsPath,
		worldName: args.name,
		sizeBytes: args.zipBytes.length,
		savedAt: args.savedAt
	};
	// The bytes are already safely in OPFS; losing the recents entry only
	// costs Resume, so don't fail the whole save over it.
	try {
		await putRecent(rec);
	} catch {
		return { persisted: false, quota: false };
	}
	return { persisted: true, quota: false };
}

export async function restoreMostRecent(
	loadZip: (bytes: Uint8Array) => void
): Promise<{ restored: boolean; needsPermission: boolean }> {
	const rec = await getMostRecent();
	if (!rec) return { restored: false, needsPermission: false };

	if (rec.kind === 'handle' && rec.handle) {
		const ok = await ensureReadWrite(rec.handle);
		if (!ok) return { restored: false, needsPermission: true };
		try {
			const entries = await readSaveFolder(rec.handle);
			setActiveDirectory(rec.handle, true);
			loadZip(zipEntries(entries));
			return { restored: true, needsPermission: false };
		} catch {
			await removeRecent(rec.id);
			return { restored: false, needsPermission: false };
		}
	}

	if (rec.kind === 'opfs' && rec.opfsPath) {
		const bytes = await getBlob(rec.opfsPath);
		if (!bytes) {
			await removeRecent(rec.id);
			return { restored: false, needsPermission: false };
		}
		loadZip(bytes);
		return { restored: true, needsPermission: false };
	}
	return { restored: false, needsPermission: false };
}

export { getMostRecent as hasRecent };
