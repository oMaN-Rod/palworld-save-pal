// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { TreeNode } from '$lib/breeding/dendrogram/types';

vi.mock('$lib/breeding/dendrogram/DendrogramEngine', () => ({
	// d3 measures the SVG it is handed, and jsdom lays nothing out.
	DendrogramEngine: class {
		hoveredId: string | null = null;
		callbacks: Record<string, unknown> = {};
		passiveName = (asset: string) => asset;
		matchedPassives: unknown = null;
		render() {}
		fit() {}
		destroy() {}
		setHovered() {}
		setSelected() {}
		zoomBy() {}
		hitTestNode() {
			return null;
		}
	}
}));

vi.mock('$states', () => ({
	getToastState: () => ({ add: vi.fn() }),
	theme: { current: 'dark' }
}));

const { default: ChainDendrogram } = await import('../ChainDendrogram.svelte');

const TREE: TreeNode = {
	id: 'root',
	tribe: 'Lamball',
	display: 'Lamball',
	children: []
} as unknown as TreeNode;

function renderTree() {
	render(ChainDendrogram, { props: { treeNode: TREE, palMap: new Map() } });
}

beforeEach(() => {
	vi.clearAllMocks();
	vi.stubGlobal(
		'ResizeObserver',
		class {
			observe() {}
			disconnect() {}
		}
	);
});

describe('ChainDendrogram touch handling', () => {
	it('takes the gesture on the tree itself', async () => {
		renderTree();
		await tick();

		const svg = screen.getByRole('application');
		expect(svg.style.touchAction).toBe('none');
	});

	it('leaves every ancestor scrollable', async () => {
		renderTree();
		await tick();

		let node = screen.getByRole('application').parentElement;
		while (node) {
			expect(node.style.touchAction).toBe('');
			node = node.parentElement;
		}
	});

	it('keeps the zoom controls reachable beside it', async () => {
		renderTree();
		await tick();

		expect(screen.getByTitle('Zoom in')).not.toBeNull();
		expect(screen.getByTitle('Zoom out')).not.toBeNull();
	});
});
