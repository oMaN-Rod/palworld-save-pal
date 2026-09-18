import { describe, expect, it } from 'vitest';
import { modPageUrl, nexusFileLabel, safeHttpUrl } from './nexusSafety';
import type { NexusFile } from '$types';

function file(overrides: Partial<NexusFile> = {}): NexusFile {
	return {
		file_id: 99001,
		name: 'Enhanced Visuals',
		version: '1.1.0',
		category: 'MAIN',
		date: 1705685026,
		size_in_bytes: 27973,
		uri: 'Enhanced Palworld Visuals-1-1-0-1705685026.7z',
		primary: true,
		description: null,
		...overrides
	};
}

describe('safeHttpUrl', () => {
	it('keeps an https url', () => {
		expect(safeHttpUrl('https://staticdelivery.nexusmods.com/mods/6063/1.jpg')).toBe(
			'https://staticdelivery.nexusmods.com/mods/6063/1.jpg'
		);
	});

	it('rejects a javascript url however it is spelled', () => {
		expect(safeHttpUrl('javascript:alert(1)')).toBeNull();
		expect(safeHttpUrl('JavaScript:alert(1)')).toBeNull();
		expect(safeHttpUrl('  javascript:alert(1)')).toBeNull();
	});

	it('rejects data, file and blank urls', () => {
		expect(safeHttpUrl('data:text/html;base64,PHN2Zz4=')).toBeNull();
		expect(safeHttpUrl('file:///C:/Windows/System32/drivers/etc/hosts')).toBeNull();
		expect(safeHttpUrl('')).toBeNull();
		expect(safeHttpUrl(null)).toBeNull();
		expect(safeHttpUrl(undefined)).toBeNull();
	});

	it('rejects a relative url, because there is no base to resolve it against', () => {
		expect(safeHttpUrl('/mods/6063/1.jpg')).toBeNull();
		expect(safeHttpUrl('staticdelivery.nexusmods.com/1.jpg')).toBeNull();
	});

	it('rejects a protocol-relative url', () => {
		expect(safeHttpUrl('//evil.com/x.jpg')).toBeNull();
	});

	it('rejects a url carrying a username or password', () => {
		expect(safeHttpUrl('https://user:pass@evil.com/x.jpg')).toBeNull();
		expect(safeHttpUrl('https://user@evil.com/x.jpg')).toBeNull();
	});
});

describe('modPageUrl', () => {
	it('builds a mod page and a file page from numbers alone', () => {
		expect(modPageUrl(4821)).toBe('https://www.nexusmods.com/palworld/mods/4821');
		expect(modPageUrl(4821, 99001)).toBe(
			'https://www.nexusmods.com/palworld/mods/4821?tab=files&file_id=99001'
		);
	});
});

describe('nexusFileLabel', () => {
	it('names a file by its name and version', () => {
		expect(nexusFileLabel(file())).toBe('Enhanced Visuals 1.1.0');
	});

	it('falls back when the version or the name is blank', () => {
		expect(nexusFileLabel(file({ version: '   ' }))).toBe('Enhanced Visuals');
		expect(nexusFileLabel(file({ name: '', version: '' }))).toBe('File 99001');
	});
});
