export async function isUpdateAvailableOnGitHub(version: string): Promise<boolean> {
    // Call GitHub API to check for latest release
    const response = await fetch('https://api.github.com/repos/oMaN-Rod/palworld-save-pal/releases/latest');
    const data = await response.json();
    const latestVersionTag = data.tag_name;

    const latestVersion = latestVersionTag.replace('v', '');

    return latestVersion !== version;
}