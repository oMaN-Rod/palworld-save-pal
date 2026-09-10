import { describe, expect, it } from 'vitest';
import { CHUNK_SEGMENT, CTL_DIRECT_MAX, ChunkAssembler, chunkFrame } from './ctlFraming';

function makeIdSource(start = 0) {
	let next = start;
	return () => next++;
}

function feed(assembler: ChunkAssembler, pieces: string[]) {
	let last: ReturnType<ChunkAssembler['accept']> | null = null;
	for (const piece of pieces) {
		last = assembler.accept(JSON.parse(piece));
	}
	return last;
}

describe('chunkFrame', () => {
	it('passes a small frame through unchanged', () => {
		const frame = JSON.stringify({ type: 'pong', data: null });
		expect(chunkFrame(frame, makeIdSource())).toEqual([frame]);
	});

	it('round-trips a large frame through the assembler', () => {
		const payload = 'x'.repeat(200 * 1024);
		const frame = JSON.stringify({ type: 'list_servers', data: payload });
		const pieces = chunkFrame(frame, makeIdSource());
		expect(pieces.length).toBeGreaterThan(1);

		const assembler = new ChunkAssembler();
		const outcome = feed(assembler, pieces);
		expect(outcome).toEqual({ kind: 'complete', text: frame });
	});

	it('keeps every chunk under the DataChannel cap even when fully escaped', () => {
		const frame = JSON.stringify({ type: 't', data: '\\"'.repeat(120 * 1024) });
		const pieces = chunkFrame(frame, makeIdSource());
		for (const piece of pieces) {
			expect(piece.length).toBeLessThanOrEqual(64 * 1024);
		}
	});

	it('never splits a surrogate pair', () => {
		const frame = JSON.stringify({ type: 't', data: '🦙'.repeat(20 * 1024) });
		const pieces = chunkFrame(frame, makeIdSource());

		const assembler = new ChunkAssembler();
		const outcome = feed(assembler, pieces);
		expect(outcome).toEqual({ kind: 'complete', text: frame });
	});

	it('keeps every chunk under the wire cap in bytes for BMP CJK text', () => {
		const frame = JSON.stringify({ type: 't', data: '漢'.repeat(30_000) });
		const pieces = chunkFrame(frame, makeIdSource());
		expect(pieces.length).toBeGreaterThan(1);

		for (const piece of pieces) {
			expect(new TextEncoder().encode(piece).length).toBeLessThanOrEqual(64 * 1024);
		}

		const assembler = new ChunkAssembler();
		const outcome = feed(assembler, pieces);
		expect(outcome).toEqual({ kind: 'complete', text: frame });
	});

	it('consumes one id per split frame', () => {
		const nextId = makeIdSource(5);
		const big = JSON.stringify({ type: 't', data: 'x'.repeat(CTL_DIRECT_MAX + 1) });
		const pieces = chunkFrame(big, nextId);
		const ids = new Set(pieces.map((p) => JSON.parse(p).data.id));
		expect(ids).toEqual(new Set([5]));
		expect(nextId()).toBe(6);
	});

	it('mirrors CHUNK_SEGMENT sizing (28 KiB)', () => {
		expect(CHUNK_SEGMENT).toBe(28 * 1024);
		expect(CTL_DIRECT_MAX).toBe(56 * 1024);
	});
});

describe('ChunkAssembler', () => {
	it('passes non-chunk envelopes straight through', () => {
		const assembler = new ChunkAssembler();
		const envelope = { type: 'list_servers' };
		expect(assembler.accept(envelope)).toEqual({ kind: 'not-chunk', envelope });
	});

	it('rejects a fifth concurrent id past the 4-id bound', () => {
		const assembler = new ChunkAssembler();
		for (let id = 0; id < 4; id++) {
			const outcome = assembler.accept({ type: 'chunk', data: { id, part: 0, parts: 2, data: 'a' } });
			expect(outcome).toEqual({ kind: 'pending' });
		}
		const fifth = assembler.accept({
			type: 'chunk',
			data: { id: 9, part: 0, parts: 2, data: 'a' }
		});
		expect(fifth).toEqual({ kind: 'rejected' });
	});

	it('rejects malformed chunk envelopes without throwing', () => {
		const assembler = new ChunkAssembler();
		const bad = [
			{ type: 'chunk' },
			{ type: 'chunk', data: { id: 0, part: 5, parts: 2, data: 'a' } },
			{ type: 'chunk', data: { id: 0, parts: 0, part: 0, data: 'a' } }
		];
		for (const envelope of bad) {
			expect(() => assembler.accept(envelope)).not.toThrow();
			expect(assembler.accept(envelope)).toEqual({ kind: 'rejected' });
		}
	});

	it('rejects a resend of an already-received part and frees its bytes', () => {
		const assembler = new ChunkAssembler();
		const first = assembler.accept({
			type: 'chunk',
			data: { id: 1, part: 0, parts: 2, data: 'a' }
		});
		expect(first).toEqual({ kind: 'pending' });

		const resend = assembler.accept({
			type: 'chunk',
			data: { id: 1, part: 0, parts: 2, data: 'a'.repeat(1_000_000) }
		});
		expect(resend).toEqual({ kind: 'rejected' });

		const retry1 = assembler.accept({
			type: 'chunk',
			data: { id: 1, part: 0, parts: 2, data: 'a' }
		});
		const retry2 = assembler.accept({
			type: 'chunk',
			data: { id: 1, part: 1, parts: 2, data: 'b' }
		});
		expect(retry1).toEqual({ kind: 'pending' });
		expect(retry2).toEqual({ kind: 'complete', text: 'ab' });
	});
});
