// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/utils/websocketUtils', () => ({ send: vi.fn(), sendAndWait: vi.fn() }));

vi.mock('$states', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$states')>();
	const { ServerState } = await import('$lib/states/serverState.svelte');
	const server = new ServerState();
	return { ...actual, getServerState: () => server };
});

import { CENTRIX } from '$lib/partners/centrixnodes';
import CreateServerModal from '../CreateServerModal.svelte';

const hostedLink = () => document.querySelector(`a[href="${CENTRIX.url}"]`);
const createButton = () => document.querySelector('[data-modal-primary]');

describe('CreateServerModal hosted option', () => {
	it('opens on the Docker form by default', () => {
		render(CreateServerModal, { closeModal: vi.fn() });

		expect(hostedLink()).toBeNull();
		expect(createButton()).not.toBeNull();
	});

	it('swaps the form for the hosting partner when Hosted is picked', async () => {
		render(CreateServerModal, { closeModal: vi.fn() });

		await fireEvent.click(screen.getByRole('button', { name: 'Hosted' }));

		expect(hostedLink()).not.toBeNull();
		expect(createButton()).toBeNull();
	});

	it('can open straight on Hosted', () => {
		render(CreateServerModal, { closeModal: vi.fn(), initialType: 'hosted' });

		expect(hostedLink()).not.toBeNull();
		expect(createButton()).toBeNull();
	});
});
