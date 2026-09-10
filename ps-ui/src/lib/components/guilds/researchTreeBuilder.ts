import { labResearchData } from '$lib/data';
import type { Guild, LabResearch, TreeNode } from '$types';

export function getGuildResearchProgress(guild: Guild, researchId: string): number {
	if (!guild || !guild.lab_research_data) return 0;
	const researchInfo = guild.lab_research_data.find(
		(r: { research_id: string }) => r.research_id === researchId
	);
	return researchInfo ? researchInfo.work_amount : 0;
}

export function getRequiredWork(researchId: string): number {
	return labResearchData.getByKey(researchId)?.details.work_amount ?? 0;
}

export function findNodeById(nodes: TreeNode[] | undefined, id: string): TreeNode | null {
	if (!nodes) return null;
	for (const node of nodes) {
		if (node.id === id) return node;
		const found = findNodeById(node.children, id);
		if (found) return found;
	}
	return null;
}

export function buildTree(
	allResearch: Record<string, LabResearch>,
	category: string,
	guild: Guild
): TreeNode[] {
	const categoryResearch = Object.values(allResearch).filter(
		(r) => r.details.category === category
	);
	const nodes: Record<string, TreeNode> = {};
	const rootNodes: TreeNode[] = [];

	categoryResearch.forEach((r) => {
		const workAmount = getGuildResearchProgress(guild, r.id);
		const totalWorkAmount = r.details.work_amount || 0;
		const isCompleted = workAmount >= totalWorkAmount && totalWorkAmount > 0;
		nodes[r.id] = {
			id: r.id,
			research: r,
			children: [],
			isUnlocked: false,
			isCompleted: isCompleted,
			workAmount: workAmount,
			totalWorkAmount: totalWorkAmount
		};
	});

	const processNodeHierarchy = (node: TreeNode) => {
		const prerequisiteId = node.research.details.require_research_id;
		if (prerequisiteId && nodes[prerequisiteId]) {
			node.isUnlocked = nodes[prerequisiteId].isCompleted;
		} else {
			node.isUnlocked = true;
		}
		node.children.forEach(processNodeHierarchy);
	};

	categoryResearch.forEach((r) => {
		const node = nodes[r.id];
		const prerequisiteId = r.details.require_research_id;
		if (prerequisiteId && nodes[prerequisiteId]) {
			nodes[prerequisiteId].children.push(node);
		} else {
			rootNodes.push(node);
		}
	});

	rootNodes.forEach(processNodeHierarchy);

	Object.values(nodes).forEach((node) =>
		node.children.sort((a, b) => a.id.localeCompare(b.id))
	);

	return rootNodes;
}