import { describe, expect, it } from 'vitest';
import { instanceErrorText, verificationLabel } from './verificationText';

describe('verificationLabel', () => {
	it('labels every verification status', () => {
		expect(verificationLabel('verified')).toBe('Verified');
		expect(verificationLabel('missing')).toBe('Not loaded');
		expect(verificationLabel('unknown')).toBe('Not checked');
		expect(verificationLabel('unexpected')).toBe('Not in profile');
	});
});

describe('instanceErrorText', () => {
	it('maps every known refusal code', () => {
		expect(instanceErrorText('target_not_found', 'raw')).toBe('That install no longer exists.');
		expect(instanceErrorText('not_found', 'raw')).toBe('That game connection no longer exists.');
		expect(instanceErrorText('remote_denied', 'raw')).toBe(
			'Game connections can only be changed on the computer running PalStudio.'
		);
	});

	it('falls back to the message for an unknown or missing code', () => {
		expect(instanceErrorText('something_else', 'raw message')).toBe('raw message');
		expect(instanceErrorText(undefined, 'raw message')).toBe('raw message');
	});
});
