export interface TreeNode {
	id: string;
	tribe: string;
	display: string;
	character_id: string;
	gender?: string | null;
	passives: string[];
	sourceType?: 'owned' | 'selected' | 'wild';
	isBred: boolean;
	isTarget?: boolean;
	stepIndex?: number;
	parents: [TreeNode, TreeNode] | null;
}

export type NodeSelectCallback = (node: TreeNode | null) => void;
export type NodeHoverCallback = (
	node: TreeNode | null,
	screenX: number,
	screenY: number
) => void;
