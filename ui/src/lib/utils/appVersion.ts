export async function isUpdateAvailableOnGitHub(version: string): Promise<boolean> {
	try {
		// Call GitHub API to check for latest release
		const response = await fetch(
			'https://api.github.com/repos/oMaN-Rod/palworld-save-pal/releases/latest'
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

function isNewerVersion(latestVersion: string, currentVersion: string): boolean {
	const partsLatest = latestVersion.split('.').map((part) => parseInt(part) || 0);
	const partsCurrent = currentVersion.split('.').map((part) => parseInt(part) || 0);

	const maxLength = Math.max(partsLatest.length, partsCurrent.length);
	while (partsLatest.length < maxLength) partsLatest.push(0);
	while (partsCurrent.length < maxLength) partsCurrent.push(0);

	for (let i = 0; i < maxLength; i++) {
		if (partsLatest[i] > partsCurrent[i]) return true; // Latest version is newer
		if (partsLatest[i] < partsCurrent[i]) return false; // Current version is newer
	}

	return false; // Versions are equal
}
