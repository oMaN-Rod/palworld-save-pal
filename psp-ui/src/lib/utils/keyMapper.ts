export function normalizeKeys(keys: string[]): Record<string, string> {
	let keyMap: Record<string, string> = {};
	for (const key of keys) {
		keyMap[key.toLowerCase()] = key;
	}
	return keyMap;
}
