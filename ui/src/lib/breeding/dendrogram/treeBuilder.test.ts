import { describe, expect, it } from 'vitest';

import { computeLayout } from './layouts';
import { directToTreeNode } from './treeBuilder';
import type { BreedablePal, DirectResultItem } from '../types';

const palMap = new Map<string, BreedablePal>();

const direct = (parent_a: string, parent_b: string, child: string): DirectResultItem => ({
	parent_a,
	parent_b,
	child,
	child_display: child,
	child_icon: null,
	child_gender_prob: null,
	combo_type: 'formula'
});

function collectIds(node: ReturnType<typeof directToTreeNode>): string[] {
	const ids: string[] = [];
	const walk = (n: typeof node) => {
		ids.push(n.id);
		n.parents?.forEach(walk);
	};
	walk(node);
	return ids;
}

describe('directToTreeNode', () => {
	it('gives distinct ids to distinct parents', () => {
		const ids = collectIds(directToTreeNode(direct('Anubis', 'Penking', 'Anubis'), palMap));
		expect(new Set(ids).size).toBe(ids.length);
	});

	// Anubis + Anubis -> Anubis rendered only one connector: both parents were
	// built with the id `Anubis#direct-leaf`, so the layout index collapsed them
	// and both links resolved to the same positioned node.
	it('gives distinct ids to a self-pair', () => {
		const ids = collectIds(directToTreeNode(direct('Anubis', 'Anubis', 'Anubis'), palMap));
		expect(new Set(ids).size).toBe(ids.length);
	});

	it('draws a separate connector to each parent of a self-pair', () => {
		const tree = directToTreeNode(direct('Anubis', 'Anubis', 'Anubis'), palMap);
		const { nodes, links, index } = computeLayout(tree, 'dendrogram');

		expect(nodes).toHaveLength(3);
		expect(index.size).toBe(3);
		expect(links).toHaveLength(2);
		expect(links[0].target.node.id).not.toBe(links[1].target.node.id);
		expect(links[0].path).not.toBe(links[1].path);
	});
});
