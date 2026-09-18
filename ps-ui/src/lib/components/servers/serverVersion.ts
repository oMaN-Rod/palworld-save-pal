function versionParts(version: string): number[] | null {
	const parts = version.trim().replace(/^v/i, '').split('.');
	if (parts.some((part) => !/^\d+$/.test(part))) return null;
	return parts.map(Number);
}

/** Compares only as many parts as `latest` has, since announcements usually omit the build number a server reports. */
export function availableUpdate(current?: string | null, latest?: string | null): string | null {
	if (!current || !latest) return null;
	const have = versionParts(current);
	const want = versionParts(latest);
	if (!have || !want) return null;
	for (let index = 0; index < want.length; index++) {
		const difference = want[index] - (have[index] ?? 0);
		if (difference !== 0) return difference > 0 ? latest : null;
	}
	return null;
}
