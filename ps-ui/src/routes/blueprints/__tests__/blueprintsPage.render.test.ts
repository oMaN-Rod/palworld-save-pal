// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { blueprintsData } = vi.hoisted(() => ({
	blueprintsData: {
		rows: [] as unknown[],
		lastImportFindings: [] as unknown[],
		list: vi.fn().mockResolvedValue(undefined)
	}
}));

vi.mock('$lib/data/blueprints.svelte', () => ({ blueprintsData }));
vi.mock('$lib/data/placement.svelte', () => ({ placementState: {} }));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$states', () => ({
	getAppState: () => ({ saveFile: undefined, guilds: {} }),
	getModalState: () => ({ showModal: vi.fn(), showConfirmModal: vi.fn() }),
	getToastState: () => ({ add: vi.fn() })
}));
vi.mock('$components/modals', () => ({ ExportBlueprintModal: {}, SelectBaseModal: {} }));

import BlueprintsPage from '../+page.svelte';

const ROW = {
	id: 'bp1',
	name: 'Ranch',
	source_world: 'World',
	structure_count: 12,
	created_at: 1700000000
};

function unprefixed(className: string | undefined) {
	return (className ?? '').split(/ +/).filter((token) => !token.includes(':'));
}

beforeEach(() => {
	vi.clearAllMocks();
	blueprintsData.rows = [ROW];
	blueprintsData.lastImportFindings = [];
});

// jsdom applies no breakpoints, so these pin the classes instead.
describe('blueprints page', () => {
	it('stacks each row over its actions below the breakpoint', async () => {
		render(BlueprintsPage);
		await tick();

		const row = document.querySelector('#blueprint-bp1');
		expect(row).not.toBeNull();
		expect(row?.className).toContain('flex-col');
		expect(row?.className).toContain('sm:flex-row');
		expect(unprefixed(row?.className)).not.toContain('items-center');
	});

	it('lets the actions wrap instead of squeezing the name', async () => {
		render(BlueprintsPage);
		await tick();

		const actions = document.querySelector('#blueprint-actions-bp1');
		expect(actions?.className).toContain('flex-wrap');
		expect(unprefixed(actions?.className)).not.toContain('shrink-0');
	});

	it('still offers every action', async () => {
		render(BlueprintsPage);
		await tick();

		expect(screen.getByText('Place')).not.toBeNull();
		expect(screen.getByText('Export .psbp')).not.toBeNull();
		expect(screen.getByText('.json')).not.toBeNull();
	});

	it('keeps the header on one line with its button', async () => {
		render(BlueprintsPage);
		await tick();

		const header = document.querySelector('#blueprints-header');
		expect(header?.className).toContain('flex-wrap');
	});
});
