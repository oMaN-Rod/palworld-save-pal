// The pick pass writes each instance's global index as a 24-bit colour. Index 0
// is reserved so the cleared (black) background decodes as "nothing hit", which
// is why the shader encodes index + 1.
export const MAX_PICK_INDEX = 0xffffff - 1;

export function decodePickBytes(r: number, g: number, b: number): number {
	const id = (r << 16) | (g << 8) | b;
	return id === 0 ? -1 : id - 1;
}
