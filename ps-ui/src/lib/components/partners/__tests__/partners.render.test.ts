// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

const { send } = vi.hoisted(() => ({ send: vi.fn() }));
vi.mock('$lib/utils/websocketUtils', () => ({ send }));

import logoBlack from '$lib/assets/img/partners/centrixnodes_black.png';
import logoWhite from '$lib/assets/img/partners/centrixnodes_white.png';
import { CENTRIX } from '$lib/partners/centrixnodes';
import { theme } from '$states';
import CentrixCallout from '../CentrixCallout.svelte';
import CentrixLogo from '../CentrixLogo.svelte';

afterEach(() => {
	theme.current = 'dark';
});

describe('CentrixLogo', () => {
	it('uses the white wordmark on a dark theme', () => {
		theme.current = 'dark';
		render(CentrixLogo);
		expect(screen.getByAltText('CentrixNodes').getAttribute('src')).toBe(logoWhite);
	});

	it('uses the black wordmark on a light theme', async () => {
		theme.current = 'lamball';
		render(CentrixLogo);
		await tick();
		expect(screen.getByAltText('CentrixNodes').getAttribute('src')).toBe(logoBlack);
	});
});

describe('CentrixCallout', () => {
	it('links to the referral page and shows the code', () => {
		const { container } = render(CentrixCallout);

		const link = container.querySelector(`a[href="${CENTRIX.url}"]`);
		expect(link).not.toBeNull();
		expect(link?.getAttribute('target')).toBe('_blank');
		expect(container.textContent).toContain(CENTRIX.code);
		expect(container.textContent).toContain(`${CENTRIX.discount}%`);
	});

	it('discloses the partnership', () => {
		const { container } = render(CentrixCallout);
		expect(container.textContent).toContain('PalStudio partner');
	});

	it('copies the code', async () => {
		const writeText = vi.fn().mockResolvedValue(undefined);
		Object.assign(navigator, { clipboard: { writeText } });
		render(CentrixCallout);

		await fireEvent.click(screen.getByRole('button', { name: /copy/i }));

		expect(writeText).toHaveBeenCalledWith(CENTRIX.code);
	});
});
