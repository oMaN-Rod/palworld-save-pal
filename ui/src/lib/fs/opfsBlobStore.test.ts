import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getBlob, putBlob, QuotaError } from './opfsBlobStore';

class FakeFile {
	constructor(public bytes: Uint8Array) {}
	async arrayBuffer() {
		return this.bytes.buffer.slice(
			this.bytes.byteOffset,
			this.bytes.byteOffset + this.bytes.byteLength
		);
	}
}
class FakeWritable {
	constructor(
		private fh: FakeFileHandle,
		private failWith?: unknown
	) {}
	async write(data: Uint8Array) {
		if (this.failWith) throw this.failWith;
		this.fh.file = new FakeFile(data);
	}
	async close() {}
}
class FakeFileHandle {
	kind = 'file' as const;
	file: FakeFile | null = null;
	constructor(
		public name: string,
		private failWith?: unknown
	) {}
	async getFile() {
		if (!this.file) throw new DOMException('NotFound', 'NotFoundError');
		return this.file;
	}
	async createWritable() {
		return new FakeWritable(this, this.failWith);
	}
}
class FakeDir {
	kind = 'directory' as const;
	entries_ = new Map<string, FakeDir | FakeFileHandle>();
	constructor(
		public name: string,
		public failNextWrite?: unknown
	) {}
	async getDirectoryHandle(name: string, opts?: { create?: boolean }) {
		let e = this.entries_.get(name);
		if (!e) {
			if (!opts?.create) throw new DOMException('NotFound', 'NotFoundError');
			e = new FakeDir(name, this.failNextWrite);
			this.entries_.set(name, e);
		}
		return e as FakeDir;
	}
	async getFileHandle(name: string, opts?: { create?: boolean }) {
		let e = this.entries_.get(name);
		if (!e) {
			if (!opts?.create) throw new DOMException('NotFound', 'NotFoundError');
			e = new FakeFileHandle(name, this.failNextWrite);
			this.entries_.set(name, e);
		}
		return e as FakeFileHandle;
	}
	async removeEntry(name: string) {
		this.entries_.delete(name);
	}
}

let opfsRoot: FakeDir;
beforeEach(() => {
	opfsRoot = new FakeDir('root');
	vi.stubGlobal('navigator', {
		storage: { getDirectory: async () => opfsRoot }
	});
});

describe('opfsBlobStore', () => {
	it('round-trips bytes', async () => {
		await putBlob('sess1.zip', new Uint8Array([7, 8, 9]));
		const back = await getBlob('sess1.zip');
		expect(back && Array.from(back)).toEqual([7, 8, 9]);
	});

	it('getBlob returns null for a missing path', async () => {
		expect(await getBlob('nope.zip')).toBeNull();
	});

	it('throws QuotaError and cleans up on quota exhaustion', async () => {
		opfsRoot.failNextWrite = new DOMException('quota', 'QuotaExceededError');
		await expect(putBlob('big.zip', new Uint8Array([1]))).rejects.toBeInstanceOf(QuotaError);
		// Partial file removed → the psp-saves dir has no 'big.zip'.
		const dir = await opfsRoot.getDirectoryHandle('psp-saves', { create: true });
		expect((dir as FakeDir).entries_.has('big.zip')).toBe(false);
	});
});
