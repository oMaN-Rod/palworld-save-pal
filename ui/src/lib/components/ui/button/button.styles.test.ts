import { describe, expect, it } from 'vitest';
import { buttonClasses } from './button.styles';

describe('buttonClasses', () => {
	it('always includes the base btn class', () => {
		expect(buttonClasses({}).split(' ')).toContain('btn');
	});

	it('defaults to neutral variant + md size', () => {
		const c = buttonClasses({});
		expect(c).toContain('btn-neutral');
		expect(c).not.toContain('btn-md'); // md uses base padding, no modifier
	});

	it('maps each variant to its class', () => {
		expect(buttonClasses({ variant: 'primary' })).toContain('btn-primary');
		expect(buttonClasses({ variant: 'secondary' })).toContain('btn-secondary');
		expect(buttonClasses({ variant: 'danger' })).toContain('btn-danger');
		expect(buttonClasses({ variant: 'ghost' })).toContain('btn-ghost');
		expect(buttonClasses({ variant: 'outline' })).toContain('btn-outline');
	});

	it('maps non-default sizes to a modifier class', () => {
		expect(buttonClasses({ size: 'sm' })).toContain('btn-sm');
		expect(buttonClasses({ size: 'lg' })).toContain('btn-lg');
		expect(buttonClasses({ size: 'icon' })).toContain('btn-icon');
	});
});
