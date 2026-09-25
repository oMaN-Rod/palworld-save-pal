// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

globalThis.ResizeObserver ??= class {
	observe() {}
	unobserve() {}
	disconnect() {}
} as unknown as typeof ResizeObserver;

vi.mock('$lib/breeding/api', () => ({
	breedingApi: { breedingPals: vi.fn().mockResolvedValue({ pals: [] }) }
}));
vi.mock('$lib/data/passiveSkills.svelte', () => ({
	passiveSkillsData: { getByKey: vi.fn(), reset: vi.fn().mockResolvedValue(undefined) }
}));
vi.mock('$states/appState.svelte', () => ({ getAppState: () => ({ saveFile: undefined }) }));
vi.mock('$lib/utils/websocketUtils', () => ({ send: vi.fn(), sendAndWait: vi.fn() }));
vi.mock('$lib/components/seo', async () => ({
	Seo: (await import('./fixtures/Empty.svelte')).default
}));

const { setViewport } = installViewportStub();
const { default: BreedingPage } = await import('../+page.svelte');

async function openGraph() {
	render(BreedingPage);
	await fireEvent.click(await screen.findByRole('button', { name: /graph/i }));
}

beforeEach(() => setViewport(1440));

describe('breeding graph controls', () => {
	it('keeps the controls beside the graph on a desktop', async () => {
		await openGraph();

		expect(screen.queryByRole('button', { name: 'Controls' })).toBeNull();
		expect(screen.getByText('Controls')).not.toBeNull();
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('moves the controls into a bottom sheet on a phone', async () => {
		setViewport(390);
		await openGraph();

		expect(screen.queryByRole('dialog')).toBeNull();
		await fireEvent.click(await screen.findByRole('button', { name: 'Controls' }));

		const sheet = screen.getByRole('dialog');
		expect(within(sheet).getByText('Controls')).not.toBeNull();
		expect(within(sheet).queryByTitle(/collapse/i)).toBeNull();
	});
});
