let handle: FileSystemDirectoryHandle | null = null;
let writable = false;
let target: 'download' | 'folder' = 'download';

export function setActiveDirectory(h: FileSystemDirectoryHandle | null, w: boolean): void {
	handle = h;
	writable = w;
}

export function getActiveDirectory(): { handle: FileSystemDirectoryHandle | null; writable: boolean } {
	return { handle, writable };
}

export function clearActiveDirectory(): void {
	handle = null;
	writable = false;
	target = 'download';
}

export function setSaveTarget(t: 'download' | 'folder'): void {
	target = t;
}

export function takeSaveTarget(): 'download' | 'folder' {
	const t = target;
	target = 'download';
	return t;
}
