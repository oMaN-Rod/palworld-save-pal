import { describe, expect, it } from 'vitest';
import { availableUpdate } from './serverVersion';

describe('availableUpdate', () => {
	it('offers a newer announced version', () => {
		expect(availableUpdate('v1.0.2.101103', 'v1.0.4')).toBe('v1.0.4');
		expect(availableUpdate('v1.0.2.100993', 'v1.0.2.101103')).toBe('v1.0.2.101103');
	});

	it('ignores the build number the announcement leaves out', () => {
		expect(availableUpdate('v1.0.4.102642', 'v1.0.4')).toBeNull();
	});

	it('offers nothing when the server is ahead', () => {
		expect(availableUpdate('v1.0.4.102642', 'v1.0.2.101103')).toBeNull();
	});

	it('offers nothing when either version is missing or unparseable', () => {
		expect(availableUpdate(null, 'v1.0.4')).toBeNull();
		expect(availableUpdate('v1.0.4', undefined)).toBeNull();
		expect(availableUpdate('v1.0.x', 'v1.0.4')).toBeNull();
	});
});
