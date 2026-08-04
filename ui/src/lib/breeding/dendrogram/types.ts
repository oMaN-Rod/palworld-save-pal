/**
 * Dendrogram node — the binary-tree shape produced by `chainToTree()` and
 * consumed by `DendrogramEngine.render()`. A binary tree: every bred node has
 * exactly two parents; leaves come from `Chain.sources`.
 *
 * The `id` is unique across the whole tree (a tribe can appear as multiple
 * distinct leaves, e.g. self-breeds, so we suffix with a counter).
 */
export interface TreeNode {
	id: string;
	tribe: string;
	display: string;
	/** Character id for icon resolution via assetLoader. Falls back to tribe. */
	character_id: string;
	/** Male | Female | null (Wildcard / bred children). */
	gender?: string | null;
	passives: string[];
	/** Leaf-only: where this pal comes from (owned / selected / wild). */
	sourceType?: 'owned' | 'selected' | 'wild';
	/** True for internal nodes (has two parents). */
	isBred: boolean;
	/** True for the root node (the chain's target pal). */
	isTarget?: boolean;
	/** For bred nodes — the index into `Chain.steps` that produced this node. */
	stepIndex?: number;
	/** Two parents for bred nodes, null for leaves. Order: [parent_a, parent_b]. */
	parents: [TreeNode, TreeNode] | null;
}

export type NodeSelectCallback = (node: TreeNode | null) => void;
export type NodeHoverCallback = (
	node: TreeNode | null,
	screenX: number,
	screenY: number
) => void;
