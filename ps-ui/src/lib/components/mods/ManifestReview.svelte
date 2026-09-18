<script module lang="ts">
	import type { FileRoute } from '$types';

	export interface TreeNode {
		name: string;
		path: string;
		file: boolean;
		children: TreeNode[];
	}

	const UNVERSIONED = 'unversioned';

	const kindOrder = [
		'ue4ss',
		'palschema',
		'pak',
		'logicmods',
		'nativedll',
		'workshop',
		'framework',
		'companion',
		'passthrough'
	];

	function sortNodes(nodes: TreeNode[]): TreeNode[] {
		nodes.sort((a, b) => Number(a.file) - Number(b.file) || a.name.localeCompare(b.name));
		for (const node of nodes) sortNodes(node.children);
		return nodes;
	}

	export function buildTree(routes: FileRoute[]): TreeNode[] {
		const root: TreeNode[] = [];
		const levels = new Map<TreeNode[], Map<string, TreeNode>>();
		for (const route of routes) {
			const parts = route.rel_path.split('/').filter((part) => part.length > 0);
			let level = root;
			let path = '';
			parts.forEach((name, index) => {
				const file = index === parts.length - 1;
				path = path ? `${path}/${name}` : name;
				let known = levels.get(level);
				if (!known) {
					known = new Map();
					levels.set(level, known);
				}
				const key = `${file}:${name}`;
				let node = known.get(key);
				if (!node) {
					node = { name, path, file, children: [] };
					known.set(key, node);
					level.push(node);
				}
				level = node.children;
			});
		}
		return sortNodes(root);
	}

	export function groupRoutes(routes: FileRoute[]): { kind: string; routes: FileRoute[] }[] {
		const groups = new Map<string, FileRoute[]>();
		for (const route of routes) {
			const grouped = groups.get(route.kind);
			if (grouped) grouped.push(route);
			else groups.set(route.kind, [route]);
		}
		const rank = (kind: string) => {
			const index = kindOrder.indexOf(kind);
			return index === -1 ? kindOrder.length : index;
		};
		return [...groups]
			.sort(([a], [b]) => rank(a) - rank(b))
			.map(([kind, grouped]) => ({ kind, routes: grouped }));
	}
</script>

<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { Decision, InstallManifest } from '$types';
	import * as m from '$i18n/messages';
	import { joinList } from './applyOutcome';
	import { destinationLabel, modTypeLabel } from './modLabels';
	import { platformLabel } from './targets';

	let { manifest, decisions = [] }: { manifest: InstallManifest; decisions?: Decision[] } =
		$props();

	const headingId = $props.id();

	const groups = $derived(groupRoutes(manifest.routes));
	const showVersion = $derived(manifest.version.length > 0 && manifest.version !== UNVERSIONED);

	let openGroups = $state<Record<string, boolean>>({});
	let closedFolders = $state<Record<string, boolean>>({});

	function decisionText(decision: Decision): string {
		switch (decision.kind) {
			case 'multiple_ue4ss_roots':
				return m.mods_review_multiple_ue4ss_roots({ roots: joinList(decision.roots, 'unit') });
			case 'pak_destination':
				return m.mods_review_pak_destination({
					file: decision.file,
					destination: destinationLabel(decision.default)
				});
			case 'unplaced_files':
				return m.mods_review_unplaced_files({ files: joinList(decision.files, 'unit') });
			case 'name_conflict':
				return m.mods_review_name_conflict({ proposed: decision.proposed });
			case 'nexus_variant':
				return m.mods_review_nexus_variant();
		}
	}

	function toggleFolder(key: string) {
		closedFolders = { ...closedFolders, [key]: !closedFolders[key] };
	}
</script>

{#snippet tree(nodes: TreeNode[], kind: string)}
	<ul class="flex flex-col gap-0.5 pl-4">
		{#each nodes as node (`${node.file}:${node.path}`)}
			<li>
				{#if node.file}
					<span class="text-surface-300 flex items-center gap-1.5 font-mono text-xs">
						<Icon icon="tabler:file" size={12} class="text-surface-500 shrink-0" />
						{node.name}
					</span>
				{:else}
					{@const key = `${kind}:${node.path}`}
					<button
						type="button"
						class="hover:text-surface-100 flex items-center gap-1.5 font-mono text-xs"
						aria-expanded={!closedFolders[key]}
						onclick={() => toggleFolder(key)}
					>
						<Icon
							icon={closedFolders[key] ? 'tabler:folder' : 'tabler:folder-open'}
							size={12}
							class="text-surface-400 shrink-0"
						/>
						{node.name}
					</button>
					{#if !closedFolders[key]}
						{@render tree(node.children, kind)}
					{/if}
				{/if}
			</li>
		{/each}
	</ul>
{/snippet}

<div class="flex flex-col gap-3">
	<div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
		<span class="text-base font-semibold">{manifest.display_name}</span>
		{#if showVersion}
			<span class="text-surface-400 font-mono text-sm">
				{m.mods_review_version({ version: manifest.version })}
			</span>
		{/if}
		<span class="text-surface-400 text-sm">{modTypeLabel(manifest.mod_type)}</span>
	</div>

	{#if manifest.platform_filtered}
		<p class="text-surface-400 text-xs">
			{m.mods_review_platform_filtered({ platform: platformLabel(manifest.platform_filtered) })}
		</p>
	{/if}

	{#if decisions.length > 0}
		<section class="border-warning-500/40 bg-warning-500/10 rounded-sm border p-3">
			<h4
				id={headingId}
				class="text-warning-400 mb-1 flex items-center gap-2 text-xs font-medium uppercase"
			>
				<Icon icon="tabler:alert-triangle" size={14} />
				{m.mods_review_decisions_title()}
			</h4>
			<ul aria-labelledby={headingId} class="flex list-disc flex-col gap-1 pl-5 text-sm">
				{#each decisions as decision, index (index)}
					<li>{decisionText(decision)}</li>
				{/each}
			</ul>
		</section>
	{/if}

	{#if groups.length === 0}
		<p class="text-surface-400 text-sm">{m.mods_review_nothing_routed()}</p>
	{:else}
		<div class="flex flex-col gap-1">
			{#each groups as group (group.kind)}
				{@const open = openGroups[group.kind] ?? false}
				<section data-route-kind={group.kind} class="bg-surface-800 rounded-sm">
					<button
						type="button"
						class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm"
						aria-expanded={open}
						onclick={() => (openGroups = { ...openGroups, [group.kind]: !open })}
					>
						<Icon
							icon="tabler:chevron-down"
							size={14}
							class={open ? 'shrink-0' : 'shrink-0 -rotate-90'}
						/>
						<span class="font-medium">{destinationLabel(group.kind)}</span>
						<span class="text-surface-400 ml-auto text-xs">
							{m.mods_file_count({ count: group.routes.length })}
						</span>
					</button>
					{#if open}
						<div class="pr-3 pb-2">
							{@render tree(buildTree(group.routes), group.kind)}
						</div>
					{/if}
				</section>
			{/each}
		</div>
	{/if}
</div>
