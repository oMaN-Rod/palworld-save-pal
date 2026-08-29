import { describe, expect, it } from 'vitest';

import { computeLayout, LAYOUT_MODES, type LayoutMode } from './layouts';
import type { TreeNode } from './types';

let seq = 0;
const leaf = (tribe: string): TreeNode => ({
	id: `${tribe}#leaf${seq++}`,
	tribe,
	display: tribe,
	character_id: tribe,
	passives: [],
	isBred: false,
	parents: null
});

const bred = (tribe: string, parents: [TreeNode, TreeNode]): TreeNode => ({
	id: `${tribe}#bred${seq++}`,
	tribe,
	display: tribe,
	character_id: tribe,
	passives: [],
	isBred: true,
	parents
});

function sampleTree(): TreeNode {
	const a = bred('Mid_A', [leaf('Leaf1'), leaf('Leaf2')]);
	const b = bred('Mid_B', [leaf('Leaf3'), leaf('Leaf4')]);
	const root = bred('Target', [a, b]);
	root.isTarget = true;
	return root;
}

function turnColumns(path: string): number[] {
	return [...path.matchAll(/Q([-\d.]+),/g)].map((m) => Number(m[1]));
}

describe('computeLayout', () => {
	it.each(LAYOUT_MODES)('%s produces a node per tree node and a link per edge', (mode) => {
		const { nodes, links, index } = computeLayout(sampleTree(), mode as LayoutMode);
		expect(nodes).toHaveLength(7);
		expect(links).toHaveLength(6);
		expect(index.size).toBe(7);
		for (const link of links) {
			expect(link.path.startsWith('M')).toBe(true);
			expect(link.path).not.toMatch(/NaN|undefined/);
		}
	});

	it('gives both links leaving a node the same turn column', () => {
		const { links } = computeLayout(sampleTree(), 'dendrogram');
		const bySource = new Map<string, number[]>();
		for (const link of links) {
			for (const x of turnColumns(link.path)) {
				const bucket = bySource.get(link.source.node.id) ?? [];
				bucket.push(x);
				bySource.set(link.source.node.id, bucket);
			}
		}
		expect(bySource.size).toBeGreaterThan(0);
		for (const [sourceId, columns] of bySource) {
			const unique = [...new Set(columns.map((x) => x.toFixed(3)))];
			expect(unique, `links leaving ${sourceId} must share one turn column`).toHaveLength(1);
		}
	});

	it('places every node at a distinct position', () => {
		for (const mode of LAYOUT_MODES) {
			const { nodes } = computeLayout(sampleTree(), mode);
			const seen = new Set(nodes.map((n) => `${n.x.toFixed(2)},${n.y.toFixed(2)}`));
			expect(seen.size, `${mode} overlaps nodes`).toBe(nodes.length);
		}
	});

	it('lays the horizontal modes out left-to-right by generation', () => {
		for (const mode of ['dendrogram', 'smooth', 'columns'] as LayoutMode[]) {
			const { index } = computeLayout(sampleTree(), mode);
			const target = [...index.values()].find((n) => n.node.isTarget);
			const leaves = [...index.values()].filter((n) => !n.node.isBred);
			expect(target).toBeDefined();
			for (const l of leaves) {
				expect(l.x, `${mode}: leaves sit right of the target`).toBeGreaterThan(target!.x);
			}
		}
	});

	it('flushes all leaves to one column in the columns view', () => {
		const { index } = computeLayout(sampleTree(), 'columns');
		const leafX = [...index.values()].filter((n) => !n.node.isBred).map((n) => n.x);
		expect(new Set(leafX).size).toBe(1);
	});

	it('uses smooth curves rather than right angles in the smooth view', () => {
		const { links } = computeLayout(sampleTree(), 'smooth');
		for (const link of links) {
			expect(link.path).toContain('C');
			expect(link.path).not.toMatch(/\sV[-\d.]/);
		}
	});

	it('spreads the radial view around the origin', () => {
		const { nodes } = computeLayout(sampleTree(), 'radial');
		expect(nodes.some((n) => n.x < 0)).toBe(true);
		expect(nodes.some((n) => n.y < 0)).toBe(true);
		expect(nodes.some((n) => n.y > 0)).toBe(true);
	});

	it('handles a childless root', () => {
		const { nodes, links } = computeLayout(leaf('Solo'), 'dendrogram');
		expect(nodes).toHaveLength(1);
		expect(links).toHaveLength(0);
	});
});
