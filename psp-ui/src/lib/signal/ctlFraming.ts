// Chunking for `ctl` messages above the DataChannel's message cap. A frame
// that fits rides as itself; a larger one splits into `chunk` envelopes that
// `ChunkAssembler` reassembles on the other side. Mirrors
// psp-server/src/signal/framing.rs, measuring in UTF-8 bytes exactly like the
// Rust side (a JS string's `.length` is UTF-16 code units, which would
// undercount multi-byte characters against the real wire-byte cap).

export const CTL_DIRECT_MAX = 56 * 1024;

export const CHUNK_SEGMENT = 28 * 1024;

const MAX_CONCURRENT_IDS = 4;
const MAX_TOTAL_BYTES = 64 * 1024 * 1024;

const encoder = new TextEncoder();

function utf8Length(codePoint: number): number {
	if (codePoint <= 0x7f) return 1;
	if (codePoint <= 0x7ff) return 2;
	if (codePoint <= 0xffff) return 3;
	return 4;
}

function exceedsByteBudget(text: string, budget: number): boolean {
	let bytes = 0;
	for (let i = 0; i < text.length; ) {
		const codePoint = text.codePointAt(i)!;
		bytes += utf8Length(codePoint);
		if (bytes > budget) return true;
		i += codePoint > 0xffff ? 2 : 1;
	}
	return false;
}

function chunkEnd(text: string, start: number): number {
	let bytes = 0;
	let index = start;
	while (index < text.length) {
		const codePoint = text.codePointAt(index)!;
		const size = utf8Length(codePoint);
		if (index > start && bytes + size > CHUNK_SEGMENT) break;
		bytes += size;
		index += codePoint > 0xffff ? 2 : 1;
	}
	return index;
}

export function chunkFrame(frame: string, nextId: () => number): string[] {
	if (!exceedsByteBudget(frame, CTL_DIRECT_MAX)) return [frame];

	const id = nextId();
	const bounds: [number, number][] = [];
	let start = 0;
	while (start < frame.length) {
		const end = chunkEnd(frame, start);
		bounds.push([start, end]);
		start = end;
	}

	const parts = bounds.length;
	return bounds.map(([from, to], part) =>
		JSON.stringify({
			type: 'chunk',
			data: { id, part, parts, data: frame.slice(from, to) }
		})
	);
}

interface PartialFrame {
	parts: (string | null)[];
	received: number;
	bytes: number;
}

export type AssemblerOutcome =
	| { kind: 'not-chunk'; envelope: unknown }
	| { kind: 'pending' }
	| { kind: 'complete'; text: string }
	| { kind: 'rejected' };

interface Chunk {
	id: number;
	part: number;
	parts: number;
	data: string;
}

function parseChunk(envelope: { type: string; data?: unknown }): Chunk | null {
	const raw = envelope.data as Record<string, unknown> | undefined;
	if (!raw || typeof raw !== 'object') return null;
	const { id, part, parts, data } = raw;
	if (typeof id !== 'number' || !Number.isInteger(id) || id < 0) return null;
	if (typeof part !== 'number' || !Number.isInteger(part) || part < 0) return null;
	if (typeof parts !== 'number' || !Number.isInteger(parts) || parts < 0) return null;
	if (parts === 0 || part >= parts) return null;
	if (typeof data !== 'string') return null;
	return { id, part, parts, data };
}

export class ChunkAssembler {
	#pending = new Map<number, PartialFrame>();
	#totalBytes = 0;

	accept(envelope: { type: string; data?: unknown }): AssemblerOutcome {
		if (envelope.type !== 'chunk') return { kind: 'not-chunk', envelope };

		const chunk = parseChunk(envelope);
		if (!chunk) return { kind: 'rejected' };

		if (!this.#pending.has(chunk.id) && this.#pending.size >= MAX_CONCURRENT_IDS) {
			return { kind: 'rejected' };
		}

		let entry = this.#pending.get(chunk.id);
		if (!entry) {
			entry = { parts: new Array<string | null>(chunk.parts).fill(null), received: 0, bytes: 0 };
			this.#pending.set(chunk.id, entry);
		}

		if (entry.parts.length !== chunk.parts || entry.parts[chunk.part] !== null) {
			this.#totalBytes -= entry.bytes;
			this.#pending.delete(chunk.id);
			return { kind: 'rejected' };
		}

		const added = encoder.encode(chunk.data).length;
		if (this.#totalBytes + added > MAX_TOTAL_BYTES) {
			this.#totalBytes -= entry.bytes;
			this.#pending.delete(chunk.id);
			return { kind: 'rejected' };
		}

		entry.received += 1;
		entry.bytes += added;
		this.#totalBytes += added;
		entry.parts[chunk.part] = chunk.data;

		if (entry.received < entry.parts.length) return { kind: 'pending' };

		this.#pending.delete(chunk.id);
		this.#totalBytes -= entry.bytes;
		return { kind: 'complete', text: entry.parts.join('') };
	}
}
