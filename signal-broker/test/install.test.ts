import { describe, expect, test } from 'vitest';
import { handleInstall, installTarget, RELEASES_URL } from '../src/install';

const CURL = 'curl/8.5.0';
const WGET = 'Wget/1.21.4 (linux-gnu)';
const WINPS = 'Mozilla/5.0 (Windows NT; Windows NT 10.0; en-US)WindowsPowerShell/5.1.19041.1682';
const PWSH_LINUX = 'Mozilla/5.0 (Linux; Ubuntu 24.04) PowerShell/7.4.1';
const PWSH_MAC = 'Mozilla/5.0 (Macintosh; macOS 14.5) PowerShell/7.4.6';
const BROWSER = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36';
// Git Bash / WSL curl says Windows (or not) but always says curl.
const CURL_ON_WINDOWS = 'curl/8.9.1 (Windows)';

describe('installTarget', () => {
	test('shell tools get the shell script', () => {
		expect(installTarget(CURL)).toBe('sh');
		expect(installTarget(WGET)).toBe('sh');
		expect(installTarget(CURL_ON_WINDOWS)).toBe('sh');
		expect(installTarget(null)).toBe('sh');
		expect(installTarget('')).toBe('sh');
	});

	test('PowerShell gets the ps1 script on every platform', () => {
		expect(installTarget(WINPS)).toBe('ps1');
		expect(installTarget(PWSH_LINUX)).toBe('ps1');
		expect(installTarget(PWSH_MAC)).toBe('ps1');
	});

	test('browsers are pointed at the releases page', () => {
		expect(installTarget(BROWSER)).toBe('browser');
	});
});

describe('handleInstall', () => {
	const get = (ua: string | null) =>
		handleInstall(new Request('https://palstudio.app/install', { headers: ua ? { 'user-agent': ua } : {} }));

	test('redirects curl to /install.sh', () => {
		const res = get(CURL);
		expect(res.status).toBe(302);
		expect(res.headers.get('location')).toBe('https://palstudio.app/install.sh');
	});

	test('redirects PowerShell to /install.ps1', () => {
		const res = get(WINPS);
		expect(res.status).toBe(302);
		expect(res.headers.get('location')).toBe('https://palstudio.app/install.ps1');
	});

	test('redirects browsers to the releases page', () => {
		const res = get(BROWSER);
		expect(res.status).toBe(302);
		expect(res.headers.get('location')).toBe(RELEASES_URL);
	});
});
