import { describe, it, expect, beforeEach } from 'vitest';
import { readSaveFolder, writeSaveInPlace } from './fileSystemAccess';

class FakeFile {
	constructor(public bytes: Uint8Array) {}
	async arrayBuffer() {
		return this.bytes.buffer.slice(this.bytes.byteOffset, this.bytes.byteOffset + this.bytes.byteLength);
	}
}
class FakeWritable {
	constructor(private fh: FakeFileHandle) {}
	async write(data: Uint8Array) {
		this.fh.file = new FakeFile(data);
	}
	async close() {}
}
class FakeFileHandle {
	kind = 'file' as const;
	constructor(public name: string, public file: FakeFile | null = null) {}
	async getFile() {
		if (!this.file) throw new Error('no file');
		return this.file;
	}
	async createWritable() {
		return new FakeWritable(this);
	}
}
class FakeDirHandle {
	kind = 'directory' as const;
	entries_ = new Map<string, FakeDirHandle | FakeFileHandle>();
	constructor(public name: string) {}
	async getDirectoryHandle(name: string, opts?: { create?: boolean }) {
		let e = this.entries_.get(name);
		if (!e) {
			if (!opts?.create) throw new DOMException('NotFound', 'NotFoundError');
			e = new FakeDirHandle(name);
			this.entries_.set(name, e);
		}
		return e as FakeDirHandle;
	}
	async getFileHandle(name: string, opts?: { create?: boolean }) {
		let e = this.entries_.get(name);
		if (!e) {
			if (!opts?.create) throw new DOMException('NotFound', 'NotFoundError');
			e = new FakeFileHandle(name);
			this.entries_.set(name, e);
		}
		return e as FakeFileHandle;
	}
	async *[Symbol.asyncIterator](): AsyncIterableIterator<[string, FakeDirHandle | FakeFileHandle]> {
		for (const [k, v] of this.entries_) yield [k, v];
	}
	entries() {
		return this[Symbol.asyncIterator]();
	}
}

function seedWorld(): FakeDirHandle {
	const root = new FakeDirHandle('world1');
	root.entries_.set('Level.sav', new FakeFileHandle('Level.sav', new FakeFile(new Uint8Array([1, 2, 3]))));
	const players = new FakeDirHandle('Players');
	players.entries_.set('abc.sav', new FakeFileHandle('abc.sav', new FakeFile(new Uint8Array([4, 5]))));
	root.entries_.set('Players', players);
	return root;
}

describe('fileSystemAccess', () => {
	let root: FakeDirHandle;
	beforeEach(() => {
		root = seedWorld();
	});

	it('readSaveFolder collects Level.sav and Players/* with forward-slash paths', async () => {
		const entries = await readSaveFolder(root as unknown as FileSystemDirectoryHandle);
		const byPath = Object.fromEntries(entries.map((e) => [e.path, Array.from(e.data)]));
		expect(byPath['Level.sav']).toEqual([1, 2, 3]);
		expect(byPath['Players/abc.sav']).toEqual([4, 5]);
	});

	it('readSaveFolder rejects a folder without Level.sav', async () => {
		const empty = new FakeDirHandle('empty');
		await expect(readSaveFolder(empty as unknown as FileSystemDirectoryHandle)).rejects.toThrow(/Level\.sav/);
	});

	it('writeSaveInPlace backs up originals before overwriting', async () => {
		const backup = await writeSaveInPlace(
			root as unknown as FileSystemDirectoryHandle,
			[{ path: 'Level.sav', bytes: new Uint8Array([9, 9]) }],
			1000
		);
		const lvl = (await root.getFileHandle('Level.sav')) as FakeFileHandle;
		expect(Array.from((await lvl.getFile()).bytes)).toEqual([9, 9]);
		// Walk each path segment — real FSA directory names cannot contain '/'.
		let bdir: FakeDirHandle = root;
		for (const seg of backup.split('/')) {
			bdir = (await bdir.getDirectoryHandle(seg)) as FakeDirHandle;
		}
		const orig = (await bdir.getFileHandle('Level.sav')) as FakeFileHandle;
		expect(Array.from((await orig.getFile()).bytes)).toEqual([1, 2, 3]);
	});

	it('resolves an existing uppercase file case-insensitively instead of creating a duplicate', async () => {
		const caseRoot = new FakeDirHandle('caseworld');
		caseRoot.entries_.set('Level.sav', new FakeFileHandle('Level.sav', new FakeFile(new Uint8Array([1]))));
		const players = new FakeDirHandle('Players');
		players.entries_.set('ABC.sav', new FakeFileHandle('ABC.sav', new FakeFile(new Uint8Array([7, 7, 7]))));
		caseRoot.entries_.set('Players', players);

		const backup = await writeSaveInPlace(
			caseRoot as unknown as FileSystemDirectoryHandle,
			[{ path: 'Players/abc.sav', bytes: new Uint8Array([8, 8]) }],
			2000
		);

		expect(players.entries_.size).toBe(1);
		expect(players.entries_.has('abc.sav')).toBe(false);
		const target = players.entries_.get('ABC.sav') as FakeFileHandle;
		expect(target).toBeDefined();
		expect(Array.from((await target.getFile()).bytes)).toEqual([8, 8]);

		let bdir: FakeDirHandle = caseRoot;
		for (const seg of backup.split('/')) {
			bdir = (await bdir.getDirectoryHandle(seg)) as FakeDirHandle;
		}
		const backupPlayers = (await bdir.getDirectoryHandle('Players')) as FakeDirHandle;
		const backedUp = Array.from(backupPlayers.entries_.values())[0] as FakeFileHandle;
		expect(Array.from((await backedUp.getFile()).bytes)).toEqual([7, 7, 7]);
	});
});
