<script lang="ts">
	// Breeding Calculator — three modes behind one page:
	//   • Direct    — A+B → child, and A+target → B options
	//   • Selection — chain to a target from a user-picked theoretical pool
	//   • Save      — chain to a target using the loaded save's owned pals
	//
	// Ported from PalSavTools. Save Mode reads owned pals from
	// `appState.selectedPlayer.pals` (already parsed by PSP's Rust core) and
	// sends them as `origin: "owned"` inputs — unifying save + selection on the
	// backend solver.
	import { onMount } from 'svelte';
	import * as m from '$i18n/messages';
	import { getAppState } from '$states/appState.svelte';
	import { isWebBuild } from '$lib/utils/platform';
	import { send } from '$lib/utils/websocketUtils';
	import { MessageType } from '$types';
	import { breedingApi } from '$lib/breeding/api';
	import { passiveSkillsData } from '$lib/data/passiveSkills.svelte';
	import { palSkillName } from '$lib/utils/breedingHelpers';
	import Spinner from '$lib/components/ui/spinner/Spinner.svelte';
	import GitMerge from '@lucide/svelte/icons/git-merge';
	import ArrowRightLeft from '@lucide/svelte/icons/arrow-right-left';
	import ListChecks from '@lucide/svelte/icons/list-checks';
	import Database from '@lucide/svelte/icons/database';
	import List from '@lucide/svelte/icons/list';
	import Play from '@lucide/svelte/icons/play';
	import Route from '@lucide/svelte/icons/route';
	import AlertTriangle from '@lucide/svelte/icons/triangle-alert';
	import Ban from '@lucide/svelte/icons/ban';
	import SearchX from '@lucide/svelte/icons/search-x';
	import X from '@lucide/svelte/icons/x';
	import PalPicker from '$lib/components/breeding/PalPicker.svelte';
	import OwnerSelect from '$lib/components/breeding/OwnerSelect.svelte';
	import DirectResult from '$lib/components/breeding/DirectResult.svelte';
	import ChainCard from '$lib/components/breeding/ChainCard.svelte';
	import GraphView from '$lib/components/breeding/GraphView.svelte';
	import BreedingSidePanel from '$lib/components/breeding/BreedingSidePanel.svelte';
	import type { TreeNode } from '$lib/breeding/dendrogram/types';
	import type { LayoutMode } from '$lib/breeding/dendrogram/layouts';
	import { directToTreeNode, chainToTree } from '$lib/breeding/dendrogram/treeBuilder';
	import type {
		BreedablePal,
		Chain as ChainT,
		ChainResponse,
		ChainRequest,
		DirectChildResponse,
		DirectParentsResponse,
		DirectPartnersResponse,
		PalInput,
		PlayerSummaryT
	} from '$lib/breeding/types';
	import type { Pal, PlayerSummary } from '$types';

	const appState = getAppState();

	type Mode = 'direct' | 'selection' | 'save';
	type DirectSub = 'forward' | 'reverse' | 'parents';

	let mode = $state<Mode>('direct');
	let directSub = $state<DirectSub>('forward');

	// Shared pal metadata cache (loaded once, shared with all pickers).
	let pals = $state<BreedablePal[]>([]);
	let palMap = $state<Map<string, BreedablePal>>(new Map());
	let palsLoading = $state(true);

	// direct mode inputs
	let parentA = $state<string | null>(null);
	let parentB = $state<string | null>(null);
	let directTarget = $state<string | null>(null);
	let directResult = $state<DirectChildResponse | null>(null);
	let partnersResult = $state<DirectPartnersResponse | null>(null);
	let parentsResult = $state<DirectParentsResponse | null>(null);

	// chain inputs
	let chainTarget = $state<string | null>(null);
	let chainGender = $state<string | null>(null);
	let chainGens = $state(4);
	let chainMaxResults = $state(5);

	// selection-specific
	let selectedPool = $state<{ tribe: string; gender: string | null }[]>([]);

	// save-specific
	let ownerUid = $state<string | null>(null);
	let includeWild = $state(false);

	// Build the breedable-player-summary list from appState for Save Mode.
	const players = $derived.by<PlayerSummaryT[]>(() => {
		const summaries = Object.values(appState.playerSummaries ?? {}) as PlayerSummary[];
		return summaries.map((p) => ({
			uid: p.uid,
			name: p.nickname,
			pal_count: p.pal_count,
			guild_name: appState.guildSummaries?.[p.guild_id ?? '']?.name
		}));
	});

	// results — scoped per chain mode so Selection and Save never share state.
	// Switching modes shows only that mode's own results; one mode's search can
	// never bleed into the other.
	interface ChainResults {
		chains: ChainT[];
		elapsedMs: number | null;
		warnings: string[];
	}
	let chainResults = $state<Record<'selection' | 'save', ChainResults>>({
		selection: { chains: [], elapsedMs: null, warnings: [] },
		save: { chains: [], elapsedMs: null, warnings: [] }
	});
	const chainModeKey = $derived(mode === 'save' ? ('save' as const) : ('selection' as const));
	const chains = $derived(chainResults[chainModeKey].chains);
	const chainElapsedMs = $derived(chainResults[chainModeKey].elapsedMs);
	const chainWarnings = $derived(chainResults[chainModeKey].warnings);
	let computing = $state(false);
	let directLoading = $state(false);
	let error = $state<string | null>(null);

	type ChainViewMode = 'list' | 'graph';
	let chainViewMode = $state<ChainViewMode>('list');
	let activeChainIndex = $state(0);
	let selectedTreeNode = $state<TreeNode | null>(null);

	type NodeDetail = {
		id: string;
		tribe: string;
		display: string;
		gender?: string | null;
		passives: string[];
		sourceType?: string;
		isBred: boolean;
		isTarget?: boolean;
		stepIndex?: number;
	};
	const selectedNodeDetail = $derived.by<NodeDetail | null>(() => {
		const n = selectedTreeNode;
		if (!n) return null;
		return {
			id: n.id,
			tribe: n.tribe,
			display: n.display,
			gender: n.gender,
			passives: n.passives,
			sourceType: n.sourceType,
			isBred: n.isBred,
			isTarget: n.isTarget,
			stepIndex: n.stepIndex
		};
	});

	type GraphLayout = 'all-in-one' | 'per-gen';
	let graphLayout = $state<GraphLayout>('all-in-one');
	let currentGen = $state(1);
	let sidePanelCollapsed = $state(false);
	// Shared by the Direct and Chain graph panes so switching mode keeps the view.
	let graphViewMode = $state<LayoutMode>('dendrogram');

	const chainTrees = $derived<TreeNode[]>(
		chains.map((c) => {
			const depth = graphLayout === 'per-gen' ? currentGen : undefined;
			return chainToTree(c, palMap, depth);
		})
	);
	const maxDepth = $derived(chains.length > 0 ? Math.max(...chains.map((c) => c.generations)) : 1);

	// A pair normally has one outcome. The unique combos the game gates on
	// parent gender (CatMage + FoxMage) have two, so render every row the
	// backend returned rather than just the headline one.
	const directForwardResults = $derived(
		directResult?.results ?? (directResult?.result ? [directResult.result] : [])
	);

	const directTrees = $derived.by<TreeNode[]>(() => {
		if (directSub === 'forward' && directForwardResults.length) {
			return directForwardResults.map((r) => directToTreeNode(r, palMap));
		}
		if (directSub === 'reverse' && partnersResult?.partners.length) {
			return partnersResult.partners.map((p) => directToTreeNode(p, palMap));
		}
		if (directSub === 'parents' && parentsResult?.parents.length) {
			return parentsResult.parents.map((p) => directToTreeNode(p, palMap));
		}
		return [];
	});

	$effect(() => {
		void chains.length;
		activeChainIndex = 0;
		selectedTreeNode = null;
	});

	onMount(async () => {
		try {
			const res = await breedingApi.breedingPals();
			pals = res.pals;
			palMap = new Map(res.pals.map((p) => [p.tribe, p]));
			// Ensure the passive catalog is loaded for display-name resolution.
			void passiveSkillsData.getByKey;
			await passiveSkillsData.reset().catch(() => {});
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			palsLoading = false;
		}
	});

	function passiveName(asset: string): string {
		return palSkillName(asset);
	}

	async function runDirect() {
		directLoading = true;
		error = null;
		// Clear the stale answer so the previous result isn't shown while the
		// new one is being computed.
		directResult = null;
		partnersResult = null;
		parentsResult = null;
		try {
			if (directSub === 'forward' && parentA && parentB) {
				directResult = await breedingApi.breedingDirectChild({
					parent_a: parentA,
					parent_b: parentB
				});
			} else if (directSub === 'reverse' && parentA && directTarget) {
				partnersResult = await breedingApi.breedingDirectPartners({
					parent_a: parentA,
					target_child: directTarget
				});
			} else if (directSub === 'parents' && directTarget) {
				parentsResult = await breedingApi.breedingDirectParents({ target_child: directTarget });
			}
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			directLoading = false;
		}
	}

	async function runChain() {
		if (!chainTarget) return;
		computing = true;
		error = null;
		chainResults[chainModeKey] = { chains: [], elapsedMs: null, warnings: [] };
		try {
			// Save Mode: ensure the selected player's pals are loaded before
			// building the request — `appState.players[uid].pals` is lazily
			// populated via REQUEST_PLAYER_DETAILS.
			if (mode === 'save') {
				const loaded = await ensurePlayerLoaded(ownerUid);
				if (!loaded) {
					error = ownerUid ? m.breeding_loading_player_failed() : m.breeding_no_loaded_players();
					computing = false;
					return;
				}
			}
			const req: ChainRequest = {
				target_pal: chainTarget,
				required_passives: [],
				target_gender: chainGender,
				max_generations: chainGens,
				max_results: chainMaxResults,
				include_wild: mode === 'save' ? includeWild : false,
				pals: mode === 'save' ? saveOwnedPals() : selectionPals()
			};
			const res: ChainResponse = await breedingApi.breedingChain(req);
			chainResults[chainModeKey] = {
				chains: res.chains,
				elapsedMs: res.elapsed_ms,
				warnings: res.warnings
			};
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			computing = false;
		}
	}

	// Build owned-pal inputs from loaded players' pals (Save Mode).
	// Reads from `appState.players` (the full player map), not `selectedPlayer`
	// (which is the editor's pick and unrelated to the breeding owner dropdown).
	// When `ownerUid` is null, uses ALL loaded players' pals.
	function saveOwnedPals(): PalInput[] {
		const playerMap = appState.players ?? {};
		const players = ownerUid
			? playerMap[ownerUid]
				? [playerMap[ownerUid]]
				: []
			: Object.values(playerMap);
		const pals: Pal[] = [];
		for (const player of players) {
			if (player?.pals) {
				pals.push(...Object.values(player.pals));
			}
		}
		return pals.map((p) => ({
			character_id: p.character_id,
			gender: p.gender,
			passive_skills: p.passive_skills,
			origin: 'owned' as const,
			instance_id: p.instance_id,
			nickname: p.nickname,
			level: p.level,
			owner_uid: p.owner_uid
		}));
	}

	// Ensure a player's full details (incl. pals) are loaded before solving.
	// `appState.players[uid].pals` is only populated after the backend answers
	// REQUEST_PLAYER_DETAILS with GET_PLAYER_DETAILS_RESPONSE (which the ws
	// dispatcher applies to `appState.players[uid]`).
	async function ensurePlayerLoaded(uid: string | null): Promise<boolean> {
		if (!uid) {
			// "All Players" — succeed if at least one player already has pals.
			return Object.values(appState.players ?? {}).some((p) => p?.pals);
		}
		if (appState.players?.[uid]?.pals) return true;
			// `sendAndWait` can't be used here: its queue resolves only when a
			// message of the REQUEST's own type arrives, but the backend answers
			// under GET_PLAYER_DETAILS_RESPONSE — so the await would never resolve
			// and Save Mode would hang. Fire the request and poll for the pals.
			send(MessageType.REQUEST_PLAYER_DETAILS, { player_id: uid, origin: 'breeding' });
			const deadline = Date.now() + 20_000;
			while (Date.now() < deadline) {
				if (appState.players?.[uid]?.pals) return true;
				await new Promise((r) => setTimeout(r, 120));
			}
			return false;
	}

	// Clear only the Save Mode result slot (owner changes invalidate results).
	function clearSaveResults() {
		chainResults.save = { chains: [], elapsedMs: null, warnings: [] };
	}

	// Build selection inputs from the user pool (Selection Mode).
	function selectionPals(): PalInput[] {
		return selectedPool.map((p) => ({
			character_id: p.tribe,
			gender: p.gender,
			passive_skills: [],
			origin: 'selected' as const
		}));
	}

	function addToPool(tribe: string) {
		if (!selectedPool.some((p) => p.tribe === tribe)) {
			selectedPool = [...selectedPool, { tribe, gender: null }];
		}
	}
	function removeFromPool(tribe: string) {
		selectedPool = selectedPool.filter((p) => p.tribe !== tribe);
	}
	function setPoolGender(tribe: string, gender: string | null) {
		selectedPool = selectedPool.map((p) => (p.tribe === tribe ? { ...p, gender } : p));
	}

	function switchMode(m: Mode) {
		mode = m;
		error = null;
		// Results are scoped per mode (chainResults), so a switch never shows
		// the other mode's chains. Reset the view state for the fresh mode.
		activeChainIndex = 0;
		selectedTreeNode = null;
	}

	// Shared active/inactive recipe for the mode + view-mode toggle pills.
	function tabPill(active: boolean): string {
		return active
			? 'bg-primary-500/15 text-primary-300 border-primary-500/40 border'
			: 'text-surface-300 hover:bg-surface-800 border border-transparent';
	}

	$effect(() => {
		void directSub;
		directResult = null;
		partnersResult = null;
		parentsResult = null;
	});

	const canRunDirect = $derived(
		directSub === 'forward'
			? !!(parentA && parentB)
			: directSub === 'reverse'
				? !!(parentA && directTarget)
				: !!directTarget
	);
	const canRunChain = $derived(
		!!chainTarget &&
			(mode === 'save' || selectedPool.length > 0) &&
			(mode !== 'save' || !!appState.saveFile)
	);

	// Count owned pals available for the current owner selection (Save Mode).
	const ownedPalCount = $derived.by<number>(() => {
		if (mode !== 'save') return 0;
		const playerMap = appState.players ?? {};
		const players = ownerUid
			? playerMap[ownerUid]
				? [playerMap[ownerUid]]
				: []
			: Object.values(playerMap);
		return players.reduce((sum, p) => sum + Object.keys(p?.pals ?? {}).length, 0);
	});

	const allTabs: { id: Mode; icon: typeof ArrowRightLeft; label: () => string }[] = [
		{ id: 'direct', icon: ArrowRightLeft, label: () => m.breeding_tabs_direct() },
		{ id: 'selection', icon: ListChecks, label: () => m.breeding_tabs_selection() },
		{ id: 'save', icon: Database, label: () => m.breeding_tabs_save() }
	];

	// Save Mode sources pals from a loaded save, so it has nothing to offer a
	// public-shell visitor. `mode` starts on 'direct', so hiding it cannot strand.
	const tabs = $derived(
		isWebBuild && !appState.saveFile ? allTabs.filter((tab) => tab.id !== 'save') : allTabs
	);
</script>

<div
	class="animate-fade-in space-y-5 p-5 {chainViewMode === 'graph'
		? 'flex h-full min-h-0 max-w-full flex-col'
		: 'mx-auto max-w-5xl'}"
>
	<!-- header -->
	<div class="flex flex-wrap items-center justify-between gap-3">
		<div class="flex items-center gap-2">
			<GitMerge size={20} class="text-primary-400" />
			<h1 class="heading-gradient text-xl font-bold">{m.breeding_title()}</h1>
		</div>
		{#if chainElapsedMs !== null}
			<span class="text-surface-400 font-mono text-xs">{chainElapsedMs}ms</span>
		{/if}
	</div>

	<!-- tab pills + list/graph toggle -->
	<div class="flex items-center gap-1.5">
		{#each tabs as tab (tab.id)}
			{@const TabIcon = tab.icon}
			<button
				class="rounded-sm flex items-center gap-1.5 px-3.5 py-2 text-sm font-medium transition-all {tabPill(mode === tab.id)}"
				onclick={() => switchMode(tab.id)}
			>
				<TabIcon size={15} />
				{tab.label()}
			</button>
		{/each}
		<div
			class="rounded-sm bg-surface-950/50 border-surface-700/40 ml-auto flex gap-1 border p-0.5"
			role="group"
			aria-label="View mode"
		>
			<button
				class="rounded-sm flex items-center gap-1 px-2.5 py-1 text-xs font-medium transition-all {tabPill(chainViewMode === 'list')}"
				onclick={() => (chainViewMode = 'list')}
			>
				<List size={12} />
				{m.breeding_view_list()}
			</button>
			<button
				class="rounded-sm flex items-center gap-1 px-2.5 py-1 text-xs font-medium transition-all {tabPill(chainViewMode === 'graph')}"
				onclick={() => (chainViewMode = 'graph')}
			>
				<GitMerge size={12} />
				{m.breeding_view_graph()}
			</button>
		</div>
	</div>

	<!-- body -->
	<div class={chainViewMode === 'graph' ? 'flex min-h-0 flex-1 flex-col' : ''}>
		{#if palsLoading}
			<div class="flex justify-center py-12"><Spinner size="size-6" /></div>
		{:else if mode === 'direct' && chainViewMode === 'list'}
			<!-- DIRECT MODE (LIST VIEW) -->
			<div class="space-y-4">
				<div class="flex gap-1.5">
					{#each [{ id: 'forward', label: m.breeding_parent_a_b() }, { id: 'reverse', label: m.breeding_parent_a_target() }, { id: 'parents', label: m.breeding_target_only() }] as sub}
						<button
							class="rounded-sm px-3.5 py-1.5 text-xs font-medium transition-all {directSub ===
							sub.id
								? 'bg-surface-800 text-surface-50 border-surface-600/60 border'
								: 'text-surface-400 hover:text-surface-200 border border-transparent'}"
							onclick={() => (directSub = sub.id as DirectSub)}
						>
							{sub.label}
						</button>
					{/each}
				</div>

				<div class="card space-y-3">
					<div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
						{#if directSub !== 'parents'}
							<div>
								<span
									class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
								>
									{m.breeding_parent_a()}
								</span>
								<PalPicker {pals} value={parentA} onselect={(t) => (parentA = t)} />
							</div>
						{/if}
						{#if directSub === 'forward'}
							<div>
								<span
									class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
								>
									{m.breeding_parent_b()}
								</span>
								<PalPicker
									{pals}
									value={parentB}
									onselect={(t) => (parentB = t)}
									exclude={parentA ? [parentA] : []}
								/>
							</div>
						{:else if directSub === 'reverse' || directSub === 'parents'}
							<div>
								<span
									class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
								>
									{m.breeding_target_child()}
								</span>
								<PalPicker {pals} value={directTarget} onselect={(t) => (directTarget = t)} />
							</div>
						{/if}
					</div>
					<button
						class="btn btn-primary text-sm"
						disabled={!canRunDirect || directLoading}
						onclick={runDirect}
					>
						{#if directLoading}<Spinner size="size-4" />{:else}<Play size={15} />{/if}
						{m.breeding_compute()}
					</button>
				</div>

				{#if error}<div
						class="rounded-md bg-error-500/10 border-error-500/30 text-error-300 flex items-center gap-1.5 border px-3 py-2 text-xs"
					>
						<AlertTriangle size={13} class="shrink-0" /><span>{error}</span>
					</div>{/if}

				{#if directSub === 'forward' && directResult}
					<div class="space-y-2">
						<h3 class="text-surface-400 text-xs font-semibold tracking-wider uppercase">
							{m.breeding_result()}
						</h3>
						{#if directForwardResults.length}
							<div class="breed-list">
								{#each directForwardResults as row (`${row.child}-${row.parent_a_gender ?? ''}`)}
									<DirectResult result={row} {palMap} />
								{/each}
							</div>
						{:else}
							<div class="text-surface-400 flex flex-col items-center justify-center gap-2 py-8">
								<Ban size={24} />
								<span class="text-xs">{m.breeding_no_combo()}</span>
							</div>
						{/if}
					</div>
				{/if}

				{#if directSub === 'reverse' && partnersResult}
					<div class="space-y-2">
						<h3 class="text-surface-400 text-xs font-semibold tracking-wider uppercase">
							{m.breeding_partners()} ({partnersResult.partners.length})
						</h3>
						{#if partnersResult.partners.length}
							<div class="breed-list">
								{#each partnersResult.partners as p, i (i)}
									<DirectResult result={p} {palMap} />
								{/each}
							</div>
						{:else}
							<div class="text-surface-400 flex flex-col items-center justify-center gap-2 py-8">
								<Ban size={24} />
								<span class="text-xs">{m.breeding_no_combo()}</span>
							</div>
						{/if}
					</div>
				{/if}

				{#if directSub === 'parents' && parentsResult}
					<div class="space-y-2">
						<h3 class="text-surface-400 text-xs font-semibold tracking-wider uppercase">
							{m.breeding_partners()} ({parentsResult.parents.length})
						</h3>
						{#if parentsResult.parents.length}
							<div class="breed-list">
								{#each parentsResult.parents as p, i (i)}
									<DirectResult result={p} {palMap} />
								{/each}
							</div>
						{:else}
							<div class="text-surface-400 flex flex-col items-center justify-center gap-2 py-8">
								<Ban size={24} />
								<span class="text-xs">{m.breeding_no_combo()}</span>
							</div>
						{/if}
					</div>
				{/if}
			</div>
		{:else if mode === 'direct' && chainViewMode === 'graph'}
			{#if error && !directTrees.length}
				<div
					class="rounded-md bg-error-500/10 border-error-500/30 text-error-300 flex items-center gap-1.5 border px-3 py-2 text-xs"
				>
					<AlertTriangle size={13} class="shrink-0" /><span>{error}</span>
				</div>
			{/if}
			<div class="flex min-h-0 flex-1 gap-4">
				<div
					class="rounded-md border-surface-700/30 bg-surface-950/20 min-h-0 min-w-0 flex-1 overflow-hidden border"
				>
					{#if directTrees.length}
						<GraphView
							trees={directTrees}
							{palMap}
							{passiveName}
							activeIndex={activeChainIndex}
							onactiveIndexChange={(idx) => (activeChainIndex = idx)}
							graphLayout={'all-in-one'}
							maxDepth={1}
							bind:viewMode={graphViewMode}
							onselect={(node) => (selectedTreeNode = node)}
						/>
					{:else}
						<div
							class="text-surface-400 flex h-full flex-1 items-center justify-center text-xs italic"
						>
							{m.breeding_select_parents_hint()}
						</div>
					{/if}
				</div>
				<div class="w-64 shrink-0">
					<div
						class="rounded-md border-surface-700/20 bg-surface-900/90 h-full overflow-hidden border shadow-xl backdrop-blur-sm"
					>
						<BreedingSidePanel
							mode="direct"
							chains={[]}
							{pals}
							activeChainIndex={0}
							{directSub}
							ondirectSubChange={(s) => (directSub = s as DirectSub)}
							{parentA}
							onparentAChange={(t) => (parentA = t)}
							{parentB}
							onparentBChange={(t) => (parentB = t)}
							{directTarget}
							ondirectTargetChange={(t) => (directTarget = t)}
							{canRunDirect}
							{directLoading}
							oncomputeDirect={runDirect}
							chainTarget={null}
							chainGender={null}
							{chainGens}
							{chainMaxResults}
							selectedPool={[]}
							players={[]}
							ownerUid={null}
							includeWild={false}
							saveLoaded={false}
							computing={false}
							canRunChain={false}
							{error}
							{palMap}
							{passiveName}
							selectedNode={selectedNodeDetail}
							collapsed={sidePanelCollapsed}
							oncollapsedChange={(v) => (sidePanelCollapsed = v)}
						/>
					</div>
				</div>
			</div>
		{:else if chainViewMode === 'list'}
			<!-- LIST MODE -->
			<div class="space-y-4">
				{#if mode === 'save' && !appState.saveFile}
					<div class="text-surface-400 flex flex-col items-center justify-center gap-2 py-12">
						<Database size={32} />
						<span class="text-sm font-medium">{m.breeding_save_required()}</span>
						<p class="text-xs">{m.breeding_save_required_hint()}</p>
					</div>
				{:else}
					<div class="card space-y-3">
						<div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
							<div>
								<span
									class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
								>
									{m.breeding_target()}
								</span>
								<PalPicker {pals} value={chainTarget} onselect={(t) => (chainTarget = t)} />
							</div>
							<div>
								<span
									class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
								>
									{m.breeding_gender()}
								</span>
								<select bind:value={chainGender} class="input text-xs">
									<option value={null}>{m.breeding_any_gender()}</option>
									<option value="Male">{m.breeding_male()}</option>
									<option value="Female">{m.breeding_female()}</option>
								</select>
							</div>
						</div>

						<div class="grid grid-cols-2 gap-3">
							<div>
								<span
									class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
								>
									{m.breeding_max_generations()}
								</span>
								<input type="number" min="1" max="6" bind:value={chainGens} class="input text-xs" />
							</div>
							<div>
								<span
									class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
								>
									{m.breeding_max_results()}
								</span>
								<input
									type="number"
									min="1"
									max="10"
									bind:value={chainMaxResults}
									class="input text-xs"
								/>
							</div>
						</div>

						{#if mode === 'selection'}
							<div>
								<span
									class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
								>
									{m.breeding_pool()} ({selectedPool.length})
								</span>
								<PalPicker
									{pals}
									placeholder={m.breeding_add_to_pool()}
									onselect={(t) => addToPool(t)}
									exclude={selectedPool.map((p) => p.tribe)}
								/>
								{#if selectedPool.length}
									<div class="mt-2 flex flex-wrap gap-1.5">
										{#each selectedPool as member (member.tribe)}
											<div
												class="rounded-sm bg-surface-950/50 border-surface-700/30 flex items-center gap-1 border px-1.5 py-0.5"
											>
												<button
													class="flex items-center gap-1 text-xs"
													onclick={() =>
														setPoolGender(
															member.tribe,
															member.gender === 'Male'
																? 'Female'
																: member.gender === 'Female'
																	? null
																	: 'Male'
														)}
													title={m.breeding_toggle_gender()}
												>
													{member.gender ?? m.breeding_any()}
												</button>
												<span class="text-surface-200 text-xs"
													>{palMap.get(member.tribe)?.display_name ?? member.tribe}</span
												>
												<button
													class="text-surface-400 hover:text-error-400 transition-colors"
													onclick={() => removeFromPool(member.tribe)}
													title={m.breeding_remove()}
												>
													<X size={10} />
												</button>
											</div>
										{/each}
									</div>
								{/if}
							</div>
						{:else}
							<!-- Save Mode: owner selector + wild toggle -->
							<div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
								<OwnerSelect
									{players}
									{ownerUid}
									onownerUidChange={(uid) => {
										ownerUid = uid;
										clearSaveResults();
									}}
								/>
								<div class="flex flex-col gap-1">
									<label class="text-surface-200 flex cursor-pointer items-center gap-2 text-xs">
										<input type="checkbox" bind:checked={includeWild} class="accent-primary-500" />
										{m.breeding_include_wild()}
									</label>
									<p class="text-surface-400 text-xs">
										{ownedPalCount > 0
											? m.breeding_owned_count({ n: ownedPalCount })
											: m.breeding_no_pals_loaded()}
									</p>
								</div>
							</div>
						{/if}

						<button
							class="btn btn-primary text-sm"
							disabled={!canRunChain || computing}
							onclick={runChain}
						>
							{#if computing}<Spinner size="size-4" />{:else}<Route size={15} />{/if}
							{m.breeding_find_chains()}
						</button>
					</div>

					{#if error}<div
							class="rounded-md bg-error-500/10 border-error-500/30 text-error-300 flex items-center gap-1.5 border px-3 py-2 text-xs"
						>
							<AlertTriangle size={13} class="shrink-0" /><span>{error}</span>
						</div>{/if}
					{#each chainWarnings as w, i (i)}
						<p class="text-warning-400 flex items-center gap-1 text-xs">
							<AlertTriangle size={13} />{w}
						</p>
					{/each}

					{#if computing && !chains.length}
						<div class="text-surface-400 flex items-center justify-center gap-2 py-12">
							<Spinner size="size-5" />
							<span class="text-sm">{m.breeding_computing()}</span>
						</div>
					{:else if chains.length}
						<div class="space-y-3">
							<h3 class="text-surface-400 text-xs font-semibold tracking-wider uppercase">
								{m.breeding_chains()} ({chains.length})
							</h3>
							{#each chains as chain, i (i)}
								<ChainCard {chain} {palMap} {passiveName} />
							{/each}
						</div>
					{:else if !computing && chainElapsedMs !== null}
						<div class="text-surface-400 flex flex-col items-center justify-center gap-2 py-12">
							<SearchX size={28} />
							<span class="text-sm">{m.breeding_no_chains()}</span>
						</div>
					{/if}
				{/if}
			</div>
		{:else}
			<!-- CHAIN GRAPH MODE -->
			{#if mode === 'save' && !appState.saveFile}
				<div class="text-surface-400 flex flex-col items-center justify-center gap-2 py-12">
					<Database size={32} />
					<span class="text-sm font-medium">{m.breeding_save_required()}</span>
					<p class="text-xs">{m.breeding_save_required_hint()}</p>
				</div>
			{:else}
				{#if error}<div
						class="rounded-md bg-error-500/10 border-error-500/30 text-error-300 flex items-center gap-1.5 border px-3 py-2 text-xs"
					>
						<AlertTriangle size={13} class="shrink-0" /><span>{error}</span>
					</div>{/if}
				{#each chainWarnings as w, i (i)}
					<p class="text-warning-400 flex items-center gap-1 text-xs">
						<AlertTriangle size={13} />{w}
					</p>
				{/each}

				<div class="flex min-h-0 flex-1 gap-4">
					<div
						class="rounded-md border-surface-700/30 bg-surface-950/20 min-h-0 min-w-0 flex-1 overflow-hidden border"
					>
						<GraphView
							trees={chainTrees}
							{chains}
							{palMap}
							{passiveName}
							activeIndex={activeChainIndex}
							onactiveIndexChange={(idx) => (activeChainIndex = idx)}
							{graphLayout}
							ongraphLayoutChange={(v) => (graphLayout = v)}
							bind:viewMode={graphViewMode}
							{currentGen}
							oncurrentGenChange={(v) => (currentGen = v)}
							{maxDepth}
							onselect={(node) => (selectedTreeNode = node)}
						/>
					</div>
					<div class="{sidePanelCollapsed ? 'w-10' : 'w-80'} shrink-0 transition-all duration-200">
						<div
							class="rounded-md border-surface-700/20 bg-surface-900/90 h-full overflow-hidden border shadow-xl backdrop-blur-sm"
						>
							<BreedingSidePanel
								mode={mode === 'save' ? 'save' : 'selection'}
								{pals}
								collapsed={sidePanelCollapsed}
								oncollapsedChange={(v) => (sidePanelCollapsed = v)}
								{chains}
								{activeChainIndex}
								onactiveChainIndexChange={(idx) => (activeChainIndex = idx)}
								{chainTarget}
								onchainTargetChange={(t) => (chainTarget = t)}
								{chainGender}
								onchainGenderChange={(g) => (chainGender = g)}
								{chainGens}
								onchainGensChange={(n) => (chainGens = n)}
								{chainMaxResults}
								onchainMaxResultsChange={(n) => (chainMaxResults = n)}
								{selectedPool}
								onaddToPool={(t) => addToPool(t)}
								onremoveFromPool={(t) => removeFromPool(t)}
								onsetPoolGender={(t, g) => setPoolGender(t, g)}
								{players}
								{ownerUid}
								onownerUidChange={(uid) => {
									ownerUid = uid;
									clearSaveResults();
								}}
								{includeWild}
								onincludeWildChange={(val) => (includeWild = val)}
								saveLoaded={!!appState.saveFile}
								{computing}
								{canRunChain}
								oncompute={runChain}
								{error}
								{palMap}
								{passiveName}
								selectedNode={selectedNodeDetail}
							/>
						</div>
					</div>
				</div>
			{/if}
		{/if}
	</div>
</div>
