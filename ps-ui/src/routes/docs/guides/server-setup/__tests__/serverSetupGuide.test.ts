import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';

import { CENTRIX } from '$lib/partners/centrixnodes';
import Guide from '../+page.md';

describe('server setup guide', () => {
	const body = render(Guide).body;

	it('lists hosting options with the partner referral', () => {
		expect(body).toContain('id="hosting-options"');
		expect(body).toContain(`href="${CENTRIX.url.replace(/&/g, '&amp;')}"`);
	});

	it('points readers behind CGNAT to hosting options', () => {
		expect(body).toContain('href="#hosting-options"');
	});
});
