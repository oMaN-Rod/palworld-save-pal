// The https://palstudio.app/install endpoint: one URL for every platform.
// curl|bash on Linux/macOS and irm|iex on Windows must each receive "their"
// script, so the route sniffs the User-Agent and redirects to the matching
// static asset (built into the site as /install.sh and /install.ps1). Humans
// clicking the link in a browser land on the releases page instead.

export type InstallTarget = 'sh' | 'ps1' | 'browser';

export const RELEASES_URL = 'https://github.com/oMaN-Rod/palworld-save-pal/releases';

// PowerShell names itself in the User-Agent on every platform, including
// pwsh on Linux. curl and wget are never browsers. Anything else
// Mozilla-flavored is a person in a browser; remaining Windows-native tools
// (irm without its PowerShell token, .NET clients) get the ps1 script.
export function installTarget(userAgent: string | null): InstallTarget {
	const ua = (userAgent ?? '').toLowerCase();
	if (ua.includes('powershell')) return 'ps1';
	if (ua.includes('curl') || ua.includes('wget')) return 'sh';
	if (ua.includes('mozilla')) return 'browser';
	if (ua.includes('windows')) return 'ps1';
	return 'sh';
}

export function handleInstall(request: Request): Response {
	switch (installTarget(request.headers.get('user-agent'))) {
		case 'browser':
			return Response.redirect(RELEASES_URL, 302);
		case 'ps1':
			return Response.redirect(new URL('/install.ps1', request.url).toString(), 302);
		default:
			return Response.redirect(new URL('/install.sh', request.url).toString(), 302);
	}
}
