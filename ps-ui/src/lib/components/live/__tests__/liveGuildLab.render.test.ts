// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import { labResearchData } from '$lib/data/labResearch.svelte';
import LiveGuildLab from '../LiveGuildLab.svelte';

beforeEach(() => {
	labResearchData.research = {
		Handcraft1: {
			id: 'Handcraft1',
			localized_name: 'Handiwork Speed Lv1',
			details: { category: 'Handcraft', work_amount: 50000, require_research_id: null, is_essential: true, icon: null }
		},
		Handcraft1_2: {
			id: 'Handcraft1_2',
			localized_name: 'Handiwork Speed Lv2',
			details: { category: 'Handcraft', work_amount: 200000, require_research_id: 'Handcraft1', is_essential: false, icon: null }
		}
	} as never;
});

const lab = {
	currentResearchId: 'Handcraft1_2',
	research: [
		{ researchId: 'Handcraft1', workAmount: 50000, requiredWorkAmount: 50000 },
		{ researchId: 'Handcraft1_2', workAmount: 100000, requiredWorkAmount: 200000 }
	]
};

describe('LiveGuildLab', () => {
	it('names research by its localized name, never by its id', async () => {
		render(LiveGuildLab, { lab });

		expect(await screen.findByLabelText('Handiwork Speed Lv1')).toBeTruthy();
		expect(screen.queryByText('Handcraft1_2')).toBeNull();
	});

	it('fades a node in proportion to its progress', async () => {
		render(LiveGuildLab, { lab });

		const inProgress = await screen.findByLabelText('Handiwork Speed Lv2');
		expect(Number(inProgress.style.opacity)).toBeCloseTo(0.62, 2);
	});

	it('leaves a finished node at full opacity', async () => {
		render(LiveGuildLab, { lab });

		const done = await screen.findByLabelText('Handiwork Speed Lv1');
		expect(Number(done.style.opacity)).toBe(1);
	});

	it('selects a node on click without completing it', async () => {
		render(LiveGuildLab, { lab });

		const node = await screen.findByLabelText('Handiwork Speed Lv2');
		await fireEvent.click(node);

		expect(screen.getByRole('heading', { name: 'Handiwork Speed Lv2' })).toBeTruthy();
		expect(Number(node.style.opacity)).toBeCloseTo(0.62, 2);
		expect(screen.queryByRole('button', { name: /unlock all/i })).toBeNull();
	});

	it('reads a guild with no lab as no research rather than failing', async () => {
		render(LiveGuildLab, { lab: null });

		expect(await screen.findByText(/0 of 2 researched/i)).toBeTruthy();
	});
});
