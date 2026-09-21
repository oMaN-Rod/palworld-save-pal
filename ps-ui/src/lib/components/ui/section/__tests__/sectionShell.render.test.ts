// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';

import SectionShellHarness from './SectionShellHarness.svelte';

describe('SectionShell', () => {
	it('renders every section body at once in columns presentation', async () => {
		render(SectionShellHarness, { presentation: 'columns' });
		await tick();

		expect(screen.getByText('active body')).not.toBeNull();
		expect(screen.getByText('passive body')).not.toBeNull();
		expect(screen.getByText('talents body')).not.toBeNull();
	});

	it('renders one collapsible per section in accordion presentation', async () => {
		render(SectionShellHarness, { presentation: 'accordion' });
		await tick();

		expect(screen.getByRole('button', { name: /active skills/i })).not.toBeNull();
		expect(screen.getByRole('button', { name: /passive skills/i })).not.toBeNull();
	});

	it('renders only the active section in tabs presentation', async () => {
		render(SectionShellHarness, { presentation: 'tabs', active: 'active' });
		await tick();

		expect(screen.getByText('active body')).not.toBeNull();
		expect(screen.queryByText('talents body')).toBeNull();
	});

	it('defines each section body exactly once regardless of presentation', async () => {
		render(SectionShellHarness, { presentation: 'columns' });
		await tick();

		expect(screen.getAllByText('active body')).toHaveLength(1);
	});
});
