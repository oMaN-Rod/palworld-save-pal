import type { BreedablePal, Chain, ChainSource, DirectResultItem } from '../types';
import type { TreeNode } from './types';

export function chainToTree(
	chain: Chain,
	palMap: Map<string, BreedablePal>,
	maxDepth?: number
): TreeNode {
	const bredNodes = new Map<number, TreeNode>();
	const leafCounter = new Map<string, number>();
	let cloneCounter = 0;

	const consumedBred = new Set<string>();

	const makeLeaf = (tribe: string, source?: ChainSource): TreeNode => {
		const n = (leafCounter.get(tribe) ?? 0) + 1;
		leafCounter.set(tribe, n);
		const pal = palMap.get(tribe);
		return {
			id: `${tribe}#leaf${n}`,
			tribe,
			display: source?.display ?? pal?.display_name ?? tribe,
			character_id: tribe,
			gender: source?.gender ?? null,
			passives: source?.passives ?? [],
			sourceType: source?.type,
			isBred: false,
			parents: null
		};
	};

	const resolveParent = (parent: {
		tribe: string;
		stepIdx?: number;
		sourceIdx?: number;
	}): TreeNode => {
		if (parent.stepIdx !== undefined) {
			const bred = bredNodes.get(parent.stepIdx);
			if (bred) {
				if (consumedBred.has(bred.id)) {
					return deepCloneNode(bred, leafCounter, cloneCounter++);
				}
				consumedBred.add(bred.id);
				return bred;
			}
		}
		if (parent.sourceIdx !== undefined) {
			const src = chain.sources[parent.sourceIdx];
			if (src) return makeLeaf(src.pal, src);
		}
		for (const bred of bredNodes.values()) {
			if (bred.tribe === parent.tribe) {
				if (consumedBred.has(bred.id)) {
					return deepCloneNode(bred, leafCounter, cloneCounter++);
				}
				consumedBred.add(bred.id);
				return bred;
			}
		}
		const src = chain.sources.find((s) => s.pal === parent.tribe);
		return makeLeaf(parent.tribe, src);
	};

	for (let i = 0; i < chain.steps.length; i++) {
		const step = chain.steps[i];
		const pal = palMap.get(step.child);
		let parentA = resolveParent({
			tribe: step.parent_a,
			stepIdx: step.parent_a_step,
			sourceIdx: step.parent_a_source
		});
		let parentB = resolveParent({
			tribe: step.parent_b,
			stepIdx: step.parent_b_step,
			sourceIdx: step.parent_b_source
		});
		if (parentA === parentB) {
			parentB = deepCloneNode(parentB, leafCounter, cloneCounter++);
		}
		const node: TreeNode = {
			id: `${step.child}#bred${i}`,
			tribe: step.child,
			display: pal?.display_name ?? step.child,
			character_id: step.child,
			gender: null,
			passives: [...step.inherited_passives],
			isBred: true,
			stepIndex: i,
			parents: [parentA, parentB]
		};
		bredNodes.set(i, node);
	}

	if (chain.steps.length === 0) {
		const src = chain.sources.find((s) => s.pal === chain.target);
		const leaf = makeLeaf(chain.target, src);
		leaf.isTarget = true;
		return leaf;
	}

	let root: TreeNode | undefined;
	for (const node of bredNodes.values()) {
		if (node.tribe === chain.target) root = node;
	}
	if (!root) {
		const leaf = makeLeaf(chain.target, chain.sources.find((s) => s.pal === chain.target));
		leaf.isTarget = true;
		return leaf;
	}
	root.isTarget = true;

	if (maxDepth !== undefined && maxDepth >= 0) {
		return pruneTree(root, maxDepth, 0);
	}

	return root;
}

function pruneTree(node: TreeNode, maxDepth: number, currentDepth: number): TreeNode {
	if (currentDepth >= maxDepth) {
		return { ...node, parents: null, isBred: false };
	}
	if (node.parents) {
		return {
			...node,
			parents: [
				pruneTree(node.parents[0], maxDepth, currentDepth + 1),
				pruneTree(node.parents[1], maxDepth, currentDepth + 1)
			]
		};
	}
	return node;
}

export function directToTreeNode(
	result: DirectResultItem,
	palMap: Map<string, BreedablePal>
): TreeNode {
	const childPal = palMap.get(result.child);
	const parentAPal = palMap.get(result.parent_a);
	const parentBPal = palMap.get(result.parent_b);

	return {
		id: `${result.child}#direct`,
		tribe: result.child,
		display: result.child_display || childPal?.display_name || result.child,
		character_id: result.child,
		gender: null,
		passives: [],
		isBred: true,
		isTarget: true,
		parents: [
			{
				id: `${result.parent_a}#direct-leaf-a`,
				tribe: result.parent_a,
				display: parentAPal?.display_name || result.parent_a,
				character_id: result.parent_a,
				gender: null,
				passives: [],
				sourceType: 'selected',
				isBred: false,
				parents: null
			},
			{
				id: `${result.parent_b}#direct-leaf-b`,
				tribe: result.parent_b,
				display: parentBPal?.display_name || result.parent_b,
				character_id: result.parent_b,
				gender: null,
				passives: [],
				sourceType: 'selected',
				isBred: false,
				parents: null
			}
		]
	};
}

function deepCloneNode(
	node: TreeNode,
	leafCounter: Map<string, number>,
	cloneTag: number
): TreeNode {
	if (node.parents) {
		return {
			...node,
			id: `${node.id}#clone${cloneTag}`,
			parents: [
				deepCloneNode(node.parents[0], leafCounter, cloneTag),
				deepCloneNode(node.parents[1], leafCounter, cloneTag)
			]
		};
	}
	const n = (leafCounter.get(node.tribe) ?? 0) + 1;
	leafCounter.set(node.tribe, n);
	return {
		...node,
		id: `${node.tribe}#clone${cloneTag}-leaf${n}`
	};
}

export function treeStats(root: TreeNode): { nodes: number; depth: number } {
	let nodes = 0;
	let depth = 0;
	const walk = (n: TreeNode, d: number) => {
		nodes++;
		depth = Math.max(depth, d);
		if (n.parents) {
			walk(n.parents[0], d + 1);
			walk(n.parents[1], d + 1);
		}
	};
	walk(root, 0);
	return { nodes, depth };
}
