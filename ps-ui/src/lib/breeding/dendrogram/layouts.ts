import { cluster, hierarchy, tree, type HierarchyPointNode } from 'd3-hierarchy';

import { DENDRO_CONFIG } from './constants';
import type { TreeNode } from './types';

export type LayoutMode = 'dendrogram' | 'smooth' | 'radial' | 'columns';

export interface PositionedNode {
	node: TreeNode;
	x: number;
	y: number;
	w: number;
}

export interface PositionedLink {
	source: PositionedNode;
	target: PositionedNode;
	path: string;
}

export interface LayoutResult {
	nodes: PositionedNode[];
	links: PositionedLink[];
	index: Map<string, PositionedNode>;
}

const nodeWidthFor = (n: TreeNode) =>
	n.isTarget === true ? DENDRO_CONFIG.targetNodeWidth : DENDRO_CONFIG.nodeWidth;

function buildHierarchy(root: TreeNode) {
	return hierarchy<TreeNode>(root, (d) => (d.parents ? [...d.parents] : []));
}

function elbowPath(sx: number, sy: number, tx: number, ty: number, midX: number, r = 8): string {
	if (Math.abs(ty - sy) < 0.5) {
		return `M${sx},${sy} H${tx}`;
	}
	const dir = ty > sy ? 1 : -1;
	const radius = Math.max(
		0,
		Math.min(r, Math.abs(midX - sx), Math.abs(tx - midX), Math.abs(ty - sy) / 2)
	);
	if (radius < 0.5) {
		return `M${sx},${sy} H${midX} V${ty} H${tx}`;
	}
	return [
		`M${sx},${sy}`,
		`H${midX - radius}`,
		`Q${midX},${sy} ${midX},${sy + radius * dir}`,
		`V${ty - radius * dir}`,
		`Q${midX},${ty} ${midX + radius},${ty}`,
		`H${tx}`
	].join(' ');
}

function curvePath(sx: number, sy: number, tx: number, ty: number): string {
	const dx = (tx - sx) * 0.5;
	return `M${sx},${sy} C${sx + dx},${sy} ${tx - dx},${ty} ${tx},${ty}`;
}

function radialPath(sx: number, sy: number, tx: number, ty: number): string {
	const mx = (sx + tx) / 2;
	const my = (sy + ty) / 2;
	const k = 0.65;
	return `M${sx},${sy} Q${mx * k},${my * k} ${tx},${ty}`;
}

function collect(laidOut: HierarchyPointNode<TreeNode>, place: (d: HierarchyPointNode<TreeNode>) => {
	x: number;
	y: number;
}): { nodes: PositionedNode[]; index: Map<string, PositionedNode> } {
	const nodes: PositionedNode[] = [];
	const index = new Map<string, PositionedNode>();
	laidOut.each((d) => {
		const { x, y } = place(d);
		const positioned: PositionedNode = { node: d.data, x, y, w: nodeWidthFor(d.data) };
		nodes.push(positioned);
		index.set(d.data.id, positioned);
	});
	return { nodes, index };
}

function horizontal(root: TreeNode, mode: LayoutMode, packLeaves: boolean): LayoutResult {
	const h = buildHierarchy(root);
	const layout = (packLeaves ? cluster<TreeNode>() : tree<TreeNode>())
		.nodeSize([
			DENDRO_CONFIG.nodeHeight + DENDRO_CONFIG.siblingGap,
			DENDRO_CONFIG.nodeWidth + DENDRO_CONFIG.levelGap
		])
		.separation((a, b) => (a.parent === b.parent ? 1 : 1.25));
	const laidOut = layout(h);

	// d3 lays out vertically and we rotate: depth -> x, sibling axis -> y.
	const { nodes, index } = collect(laidOut, (d) => ({ x: d.y, y: d.x }));

	const turnX = new Map<string, number>();
	for (const link of laidOut.links()) {
		const source = index.get(link.source.data.id);
		const target = index.get(link.target.data.id);
		if (!source || !target) continue;
		const sx = source.x + source.w;
		const span = target.x - sx;
		const existing = turnX.get(source.node.id);
		const candidate = sx + span * 0.5;
		turnX.set(source.node.id, existing === undefined ? candidate : Math.min(existing, candidate));
	}

	const links: PositionedLink[] = [];
	for (const link of laidOut.links()) {
		const source = index.get(link.source.data.id);
		const target = index.get(link.target.data.id);
		if (!source || !target) continue;
		const sx = source.x + source.w;
		const sy = source.y;
		const tx = target.x;
		const ty = target.y;
		const path =
			mode === 'smooth'
				? curvePath(sx, sy, tx, ty)
				: elbowPath(sx, sy, tx, ty, turnX.get(source.node.id) ?? sx + (tx - sx) * 0.5);
		links.push({ source, target, path });
	}

	return { nodes, links, index };
}

function radial(root: TreeNode): LayoutResult {
	const h = buildHierarchy(root);
	const depth = Math.max(1, h.height);
	const ringGap = DENDRO_CONFIG.nodeWidth + DENDRO_CONFIG.levelGap * 1.4;
	const laidOut = tree<TreeNode>()
		.size([2 * Math.PI, depth * ringGap])
		.separation((a, b) => (a.parent === b.parent ? 1 : 1.6) / Math.max(a.depth, 1))(h);

	const { nodes, index } = collect(laidOut, (d) => {
		const angle = d.x - Math.PI / 2;
		const radius = d.y;
		const cx = Math.cos(angle) * radius;
		const cy = Math.sin(angle) * radius;
		return { x: cx - nodeWidthFor(d.data) / 2, y: cy };
	});

	const links: PositionedLink[] = [];
	for (const link of laidOut.links()) {
		const source = index.get(link.source.data.id);
		const target = index.get(link.target.data.id);
		if (!source || !target) continue;
		const scx = source.x + source.w / 2;
		const tcx = target.x + target.w / 2;
		links.push({ source, target, path: radialPath(scx, source.y, tcx, target.y) });
	}

	return { nodes, links, index };
}

export function computeLayout(root: TreeNode, mode: LayoutMode): LayoutResult {
	switch (mode) {
		case 'smooth':
			return horizontal(root, 'smooth', false);
		case 'columns':
			return horizontal(root, 'columns', true);
		case 'radial':
			return radial(root);
		case 'dendrogram':
		default:
			return horizontal(root, 'dendrogram', false);
	}
}

export const LAYOUT_MODES: LayoutMode[] = ['dendrogram', 'smooth', 'columns', 'radial'];
