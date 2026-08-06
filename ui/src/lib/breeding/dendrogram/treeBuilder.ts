/**
 * Rebuild a recursive binary tree from a flattened `Chain`.
 *
 * Ported from PalSavTools — logic identical, only the type imports changed
 * (BreedablePal → BreedablePal from our breeding types; icon → character_id
 * for PSP's assetLoader). The tree is "upside down" — target is root, source
 * pals are leaves.
 */
import type { BreedablePal, Chain, ChainSource, DirectResultItem } from '../types';
import type { TreeNode } from './types';

export function chainToTree(
	chain: Chain,
	palMap: Map<string, BreedablePal>,
	maxDepth?: number
): TreeNode {
	// Bred nodes keyed by STEP INDEX (not tribe) — a species can be produced
	// by multiple steps, and each step's parents reference the exact prior
	// step index that produced them (see BreedingStep.parent_*_step).
	const bredNodes = new Map<number, TreeNode>();
	const leafCounter = new Map<string, number>();
	let cloneCounter = 0;

	// Track which bred-node IDs have already been consumed as a parent by a
	// prior step. d3-hierarchy requires a strict tree (no shared subtrees), so
	// when the same bred node is referenced by multiple subsequent steps, every
	// reference after the first MUST be a deep clone — otherwise d3 collapses
	// the shared branch and connecting lines vanish.
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

	// Resolve a parent lineage ref. Bred nodes are shared — first consumption
	// returns the original, subsequent consumptions return deep clones so each
	// usage is an independent branch in the d3 tree.
	const resolveParent = (parent: {
		tribe: string;
		stepIdx?: number;
		sourceIdx?: number;
	}): TreeNode => {
		// Explicit bred-step reference (new backend lineage refs).
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
		// Explicit source reference.
		if (parent.sourceIdx !== undefined) {
			const src = chain.sources[parent.sourceIdx];
			if (src) return makeLeaf(src.pal, src);
		}
		// Fallback (no refs — older data): tribe-based resolution.
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
		// Self-breed: both parents resolve to the same node reference (same
		// species, same bred history). Clone one so d3 sees two branches.
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

	// Root = the last step's child (the chain target), by contract of the
	// topologically-sorted steps. But resolve by target tribe defensively.
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
				// Suffixed per slot, not per tribe: a self-pair (Anubis + Anubis)
				// would otherwise give both parents one id, and the layout index
				// collapses same-id nodes into a single connector.
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
