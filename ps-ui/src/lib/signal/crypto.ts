const HKDF_SALT = new TextEncoder().encode('psp-signal-v1');
const NONCE_LEN = 12;

export function normalizeCode(input: string): string {
	return input.replace(/[-\s]/g, '').toUpperCase();
}

function bytesToHex(bytes: Uint8Array): string {
	return Array.from(bytes)
		.map((b) => b.toString(16).padStart(2, '0'))
		.join('');
}

function hexToBytes(hex: string): Uint8Array<ArrayBuffer> {
	const bytes = new Uint8Array(hex.length / 2);
	for (let i = 0; i < bytes.length; i++) {
		bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
	}
	return bytes;
}

async function deriveHkdfBaseKey(code: string): Promise<CryptoKey> {
	const ikm = new TextEncoder().encode(normalizeCode(code));
	return crypto.subtle.importKey('raw', ikm, 'HKDF', false, ['deriveBits', 'deriveKey']);
}

async function deriveHkdfBaseKeyFromBytes(ikm: Uint8Array<ArrayBuffer>): Promise<CryptoKey> {
	return crypto.subtle.importKey('raw', ikm, 'HKDF', false, ['deriveBits', 'deriveKey']);
}

async function deriveSealBits(baseKey: CryptoKey): Promise<ArrayBuffer> {
	return crypto.subtle.deriveBits(
		{ name: 'HKDF', hash: 'SHA-256', salt: HKDF_SALT, info: new TextEncoder().encode('seal') },
		baseKey,
		32 * 8
	);
}

/** Exposes the raw derived seal-key bytes for vector comparison; production
 * code should use `deriveKeys` so the AES-GCM key stays non-extractable. */
export async function deriveKeyBytes(code: string): Promise<Uint8Array> {
	const baseKey = await deriveHkdfBaseKey(code);
	return new Uint8Array(await deriveSealBits(baseKey));
}

export async function deriveKeys(code: string): Promise<{ roomId: string; sealKey: CryptoKey }> {
	const baseKey = await deriveHkdfBaseKey(code);

	const roomBits = await crypto.subtle.deriveBits(
		{ name: 'HKDF', hash: 'SHA-256', salt: HKDF_SALT, info: new TextEncoder().encode('room') },
		baseKey,
		16 * 8
	);
	const sealBits = await deriveSealBits(baseKey);
	const sealKey = await crypto.subtle.importKey('raw', sealBits, { name: 'AES-GCM' }, false, [
		'encrypt',
		'decrypt'
	]);

	return { roomId: bytesToHex(new Uint8Array(roomBits)), sealKey };
}

/** Exposes the raw derived device seal-key bytes for vector comparison;
 * production code should use `deriveDeviceSealKey` so the AES-GCM key stays
 * non-extractable. */
export async function deriveDeviceSealKeyBytes(deviceSecretHex: string): Promise<Uint8Array> {
	const baseKey = await deriveHkdfBaseKeyFromBytes(hexToBytes(deviceSecretHex));
	return new Uint8Array(await deriveSealBits(baseKey));
}

export async function deriveDeviceSealKey(deviceSecretHex: string): Promise<CryptoKey> {
	const baseKey = await deriveHkdfBaseKeyFromBytes(hexToBytes(deviceSecretHex));
	const sealBits = await deriveSealBits(baseKey);
	return crypto.subtle.importKey('raw', sealBits, { name: 'AES-GCM' }, false, [
		'encrypt',
		'decrypt'
	]);
}

export async function seal(
	key: CryptoKey,
	plaintext: Uint8Array,
	aad: Uint8Array
): Promise<Uint8Array> {
	const nonce = crypto.getRandomValues(new Uint8Array(NONCE_LEN));
	const ciphertext = new Uint8Array(
		await crypto.subtle.encrypt(
			{ name: 'AES-GCM', iv: nonce, additionalData: new Uint8Array(aad) },
			key,
			new Uint8Array(plaintext)
		)
	);

	const sealed = new Uint8Array(NONCE_LEN + ciphertext.length);
	sealed.set(nonce, 0);
	sealed.set(ciphertext, NONCE_LEN);
	return sealed;
}

export async function open(
	key: CryptoKey,
	sealed: Uint8Array,
	aad: Uint8Array
): Promise<Uint8Array> {
	if (sealed.length < NONCE_LEN) {
		throw new Error('sealed envelope is shorter than the nonce prefix');
	}
	const nonce = new Uint8Array(sealed.slice(0, NONCE_LEN));
	const ciphertext = new Uint8Array(sealed.slice(NONCE_LEN));
	const plaintext = await crypto.subtle.decrypt(
		{ name: 'AES-GCM', iv: nonce, additionalData: new Uint8Array(aad) },
		key,
		ciphertext
	);
	return new Uint8Array(plaintext);
}
