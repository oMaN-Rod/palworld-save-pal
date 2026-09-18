import { describe, expect, it } from 'vitest';
import { frameworkErrorText } from './frameworkText';

describe('frameworkErrorText', () => {
	it('joins the dependents for framework_required', () => {
		expect(
			frameworkErrorText({
				code: 'framework_required',
				message: 'm',
				framework: 'ue4ss',
				dependents: ['CoolMod', 'OtherMod']
			})
		).toContain('CoolMod, OtherMod');
	});

	it('falls back to the message for an unknown code', () => {
		expect(frameworkErrorText({ code: 'something_else', message: 'raw message' })).toBe(
			'raw message'
		);
	});

	it('reads "Install {framework} first." when framework_required has no dependents', () => {
		expect(
			frameworkErrorText({ code: 'framework_required', message: 'm', framework: 'ue4ss' })
		).toBe('Install UE4SS first.');
	});

	it('uses resolved names when given, instead of the raw dependents', () => {
		expect(
			frameworkErrorText(
				{
					code: 'framework_required',
					message: 'm',
					framework: 'ue4ss',
					dependents: ['palschema', 'cool-mod']
				},
				['PalSchema', 'Cool Mod']
			)
		).toContain('PalSchema, Cool Mod');
	});
});
