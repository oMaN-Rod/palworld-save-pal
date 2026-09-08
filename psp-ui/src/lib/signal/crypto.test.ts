import { describe, expect, it } from 'vitest';
import {
	deriveDeviceSealKey,
	deriveDeviceSealKeyBytes,
	deriveKeyBytes,
	deriveKeys,
	normalizeCode,
	open,
	seal
} from './crypto';
import vectors from './vectors.json';

function hexToBytes(hex: string): Uint8Array {
	const bytes = new Uint8Array(hex.length / 2);
	for (let i = 0; i < bytes.length; i++) {
		bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
	}
	return bytes;
}

function bytesToHex(bytes: Uint8Array): string {
	return Array.from(bytes)
		.map((b) => b.toString(16).padStart(2, '0'))
		.join('');
}

describe('normalizeCode', () => {
	it('uppercases and strips dashes and whitespace', () => {
		expect(normalizeCode('abcd-1234 efgh')).toBe('ABCD1234EFGH');
		expect(normalizeCode('  xy-z  ')).toBe('XYZ');
		expect(normalizeCode('ALREADY-NORMAL')).toBe('ALREADYNORMAL');
	});
});

describe('deriveKeys', () => {
	it('is deterministic for the same code', async () => {
		const a = await deriveKeys('ABCD-EFGH-JKMN-PQRS-TUVW-XY23');
		const b = await deriveKeys('ABCD-EFGH-JKMN-PQRS-TUVW-XY23');

		expect(a.roomId).toBe(b.roomId);
	});

	it('ignores case, dashes and whitespace', async () => {
		const a = await deriveKeys('ABCD-EFGH-JKMN-PQRS-TUVW-XY23');
		const b = await deriveKeys(' abcd efgh-jkmn pqrs-tuvw xy23 ');

		expect(a.roomId).toBe(b.roomId);
	});

	it('produces a 32-character lowercase hex room id', async () => {
		const { roomId } = await deriveKeys('SOME-PAIRING-CODE');

		expect(roomId).toMatch(/^[0-9a-f]{32}$/);
	});

	it('derives different keys for different codes', async () => {
		const a = await deriveKeys('CODE-ONE-XXXXXX');
		const b = await deriveKeys('CODE-TWO-YYYYYY');

		expect(a.roomId).not.toBe(b.roomId);
	});

	it('returns a non-extractable seal key', async () => {
		const { sealKey } = await deriveKeys('NON-EXTRACTABLE-CODE');

		expect(sealKey.extractable).toBe(false);
		await expect(crypto.subtle.exportKey('raw', sealKey)).rejects.toThrow();
	});
});

describe('seal and open round-trip', () => {
	it('opens what it sealed', async () => {
		const { sealKey } = await deriveKeys('ROUND-TRIP-CODE');
		const plaintext = new TextEncoder().encode('hello signal room');
		const aad = new TextEncoder().encode('host');

		const sealed = await seal(sealKey, plaintext, aad);
		const opened = await open(sealKey, sealed, aad);

		expect(opened).toEqual(plaintext);
	});

	it('uses a random 12-byte nonce prefix each call', async () => {
		const { sealKey } = await deriveKeys('NONCE-CODE');
		const plaintext = new TextEncoder().encode('same plaintext');
		const aad = new TextEncoder().encode('guest');

		const a = await seal(sealKey, plaintext, aad);
		const b = await seal(sealKey, plaintext, aad);

		expect(a.slice(0, 12)).not.toEqual(b.slice(0, 12));
	});

	it('throws when the ciphertext is tampered with', async () => {
		const { sealKey } = await deriveKeys('TAMPER-CODE');
		const sealed = await seal(
			sealKey,
			new TextEncoder().encode('do not touch me'),
			new TextEncoder().encode('host')
		);
		sealed[sealed.length - 1] ^= 0x01;

		await expect(open(sealKey, sealed, new TextEncoder().encode('host'))).rejects.toThrow();
	});

	it('throws when the AAD does not match the sender role', async () => {
		const { sealKey } = await deriveKeys('AAD-CODE');
		const sealed = await seal(
			sealKey,
			new TextEncoder().encode('reflected message'),
			new TextEncoder().encode('host')
		);

		await expect(open(sealKey, sealed, new TextEncoder().encode('guest'))).rejects.toThrow();
	});

	it('throws on a truncated envelope', async () => {
		const { sealKey } = await deriveKeys('TRUNCATED-CODE');

		await expect(
			open(sealKey, new Uint8Array(4), new TextEncoder().encode('host'))
		).rejects.toThrow();
	});
});

describe('shared vectors', () => {
	for (const vector of vectors.pairing) {
		it(`conforms for code ${vector.code}`, async () => {
			const { roomId, sealKey } = await deriveKeys(vector.code);

			expect(roomId).toBe(vector.roomId);

			const sealKeyBytes = await deriveKeyBytes(vector.code);
			expect(bytesToHex(sealKeyBytes)).toBe(vector.sealKeyHex);

			const sealed = hexToBytes(vector.sealedHex);
			const plaintext = hexToBytes(vector.plaintextHex);
			const aad = new TextEncoder().encode(vector.aadRole);

			const opened = await open(sealKey, sealed, aad);
			expect(opened).toEqual(plaintext);
		});
	}
});

describe('deriveDeviceSealKey', () => {
	it('returns a non-extractable AES-GCM key', async () => {
		const key = await deriveDeviceSealKey('11'.repeat(32));

		expect(key.extractable).toBe(false);
		await expect(crypto.subtle.exportKey('raw', key)).rejects.toThrow();
	});

	for (const vector of vectors.deviceSeal) {
		it(`conforms for device secret ${vector.deviceSecretHex}`, async () => {
			const keyBytes = await deriveDeviceSealKeyBytes(vector.deviceSecretHex);
			expect(bytesToHex(keyBytes)).toBe(vector.sealKeyHex);

			const key = await deriveDeviceSealKey(vector.deviceSecretHex);
			const sealed = hexToBytes(vector.sealedHex);
			const plaintext = hexToBytes(vector.plaintextHex);
			const aad = new TextEncoder().encode(vector.aadRole);

			const opened = await open(key, sealed, aad);
			expect(opened).toEqual(plaintext);
		});
	}
});
