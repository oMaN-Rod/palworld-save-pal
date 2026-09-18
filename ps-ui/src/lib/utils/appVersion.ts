export async function isUpdateAvailableOnGitHub(version: string): Promise<boolean> {
	try {
		const response = await fetch(
			'https://api.github.com/repos/oMaN-Rod/palstudio/releases/latest'
		);

		if (!response.ok) {
			throw new Error(`GitHub API error: ${response.status} ${response.statusText}`);
		}

		const data = await response.json();
		const latestVersion = data.tag_name.replace(/^v/, '');
		const currentVersion = version.replace(/^v/, '');

		return isNewerVersion(latestVersion, currentVersion);
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		throw new Error(`Failed to check for updates: ${errorMessage}`);
	}
}

export function isNewerVersion(latestVersion: string, currentVersion: string): boolean {
	const latest = parseVersion(latestVersion);
	const current = parseVersion(currentVersion);

	const fields = Math.max(latest.release.length, current.release.length);
	for (let i = 0; i < fields; i++) {
		const difference = (latest.release[i] ?? 0) - (current.release[i] ?? 0);
		if (difference !== 0) return difference > 0;
	}

	return comparePreRelease(latest.preRelease, current.preRelease) > 0;
}

function parseVersion(version: string): { release: number[]; preRelease: string[] } {
	const withoutBuild = version.split('+')[0];
	const dash = withoutBuild.indexOf('-');
	const release = dash === -1 ? withoutBuild : withoutBuild.slice(0, dash);
	const preRelease = dash === -1 ? '' : withoutBuild.slice(dash + 1);

	return {
		release: release.split('.').map((part) => parseInt(part, 10) || 0),
		preRelease: preRelease === '' ? [] : preRelease.split('.')
	};
}

function comparePreRelease(latest: string[], current: string[]): number {
	if (latest.length === 0 && current.length === 0) return 0;
	if (latest.length === 0) return 1;
	if (current.length === 0) return -1;

	for (let i = 0; i < Math.min(latest.length, current.length); i++) {
		const difference = compareIdentifier(latest[i], current[i]);
		if (difference !== 0) return difference;
	}

	return latest.length - current.length;
}

function compareIdentifier(latest: string, current: string): number {
	const latestNumeric = /^\d+$/.test(latest);
	const currentNumeric = /^\d+$/.test(current);

	if (latestNumeric && currentNumeric) return parseInt(latest, 10) - parseInt(current, 10);
	if (latestNumeric !== currentNumeric) return latestNumeric ? -1 : 1;
	if (latest === current) return 0;
	return latest < current ? -1 : 1;
}
