import { describe, expect, it } from 'vitest';
import { buildSetSourceRequest, sourceKindFromStatus } from './signalSource.utils';

describe('sourceKindFromStatus', () => {
	it('maps file to file', () => {
		expect(sourceKindFromStatus('file')).toBe('file');
	});

	it('maps rest to server', () => {
		expect(sourceKindFromStatus('rest')).toBe('server');
	});

	it('maps undefined to off', () => {
		expect(sourceKindFromStatus(undefined)).toBe('off');
	});
});

describe('buildSetSourceRequest', () => {
	it('sends {kind: off} for off', () => {
		expect(buildSetSourceRequest('off', { filePath: '', selectedServerId: '' })).toEqual({
			kind: 'off'
		});
	});

	it('sends the trimmed file path for file', () => {
		expect(
			buildSetSourceRequest('file', { filePath: ' /save/dir ', selectedServerId: '' })
		).toEqual({
			kind: 'file',
			path: '/save/dir'
		});
	});

	it('omits path for file when blank', () => {
		expect(buildSetSourceRequest('file', { filePath: '  ', selectedServerId: '' })).toEqual({
			kind: 'file',
			path: undefined
		});
	});

	it('sends the selected server id for server', () => {
		expect(buildSetSourceRequest('server', { filePath: '', selectedServerId: 3 })).toEqual({
			kind: 'server',
			server_id: 3
		});
	});

	it('sends nothing for server when no server is selected', () => {
		expect(buildSetSourceRequest('server', { filePath: '', selectedServerId: '' })).toBeNull();
	});
});
