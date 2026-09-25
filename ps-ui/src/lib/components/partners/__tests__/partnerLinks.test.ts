import * as m from '$i18n/messages';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';

import Cta from '$components/landing/Cta.svelte';
import { CENTRIX } from '$lib/partners/centrixnodes';
import PartnerCard from '../PartnerCard.svelte';

const offer = m.landing_partner_offer({ code: CENTRIX.code, discount: CENTRIX.discount });

describe('About partner card', () => {
	const body = render(PartnerCard).body;

	it('links to the referral page with the offer and disclosure', () => {
		expect(body).toContain(`href="${CENTRIX.url}"`);
		expect(body).toContain(m.about_partners_heading());
		expect(body).toContain(
			m.about_partner_offer({ code: CENTRIX.code, discount: CENTRIX.discount })
		);
		expect(body).toContain(m.about_partner_disclosure());
	});
});

describe('landing footer partner line', () => {
	const body = render(Cta).body;

	it('names the hosting partner with the offer', () => {
		expect(body).toContain(m.landing_partner_label());
		expect(body).toContain(offer);
		expect(body).toContain(`href="${CENTRIX.url}"`);
	});

	it('sits below the community links', () => {
		expect(body.indexOf('aria-label="Discord"')).toBeLessThan(body.indexOf(CENTRIX.url));
	});
});
