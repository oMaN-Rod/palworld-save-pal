<script lang="ts">
	/**
	 * BreedingSidePanel — right-hand panel for Graph Mode. Contains the chain
	 * selector tabs, configuration inputs, selection-pool or save-owner
	 * controls, the compute button, and a node-detail section.
	 */
	import { ChevronRight, ChevronLeft } from '@lucide/svelte/icons';
	import Play from '@lucide/svelte/icons/play';
	import X from '@lucide/svelte/icons/x';
	import * as m from '$i18n/messages';
	import type { BreedablePal, Chain, PlayerSummaryT } from '$lib/breeding/types';
	import PalPicker from './PalPicker.svelte';
	import OwnerSelect from './OwnerSelect.svelte';
	import PalSlot from './PalSlot.svelte';
	import Spinner from '$lib/components/ui/spinner/Spinner.svelte';

	interface NodeDetail {
		id: string;
		tribe: string;
		display: string;
		gender?: string | null;
		passives: string[];
		sourceType?: string;
		isBred: boolean;
		isTarget?: boolean;
		stepIndex?: number;
	}

	let {
		collapsed = false,
		oncollapsedChange,
		mode,
		directSub = 'forward',
		ondirectSubChange,
		parentA = null,
		onparentAChange,
		parentB = null,
		onparentBChange,
		directTarget = null,
		ondirectTargetChange,
		canRunDirect = false,
		directLoading = false,
		oncomputeDirect,
		chains,
		activeChainIndex,
		onactiveChainIndexChange,
		chainTarget,
		onchainTargetChange,
		chainGender,
		onchainGenderChange,
		chainGens,
		onchainGensChange,
		chainMaxResults,
		onchainMaxResultsChange,
		selectedPool,
		onaddToPool,
		onremoveFromPool,
		onsetPoolGender,
		players,
		ownerUid,
		onownerUidChange,
		includeWild,
		onincludeWildChange,
		saveLoaded,
		computing,
		canRunChain,
		oncompute,
		error,
		palMap,
		passiveName = (asset: string) => asset,
		selectedNode,
		pals = []
	}: {
		collapsed?: boolean;
		oncollapsedChange?: (val: boolean) => void;
		mode: 'direct' | 'selection' | 'save';
		directSub?: string;
		ondirectSubChange?: (sub: string) => void;
		parentA?: string | null;
		onparentAChange?: (t: string) => void;
		parentB?: string | null;
		onparentBChange?: (t: string) => void;
		directTarget?: string | null;
		ondirectTargetChange?: (t: string) => void;
		canRunDirect?: boolean;
		directLoading?: boolean;
		oncomputeDirect?: () => void;
		chains: Chain[];
		activeChainIndex: number;
		onactiveChainIndexChange?: (idx: number) => void;
		chainTarget: string | null;
		onchainTargetChange?: (t: string) => void;
		chainGender: string | null;
		onchainGenderChange?: (g: string | null) => void;
		chainGens: number;
		onchainGensChange?: (n: number) => void;
		chainMaxResults: number;
		onchainMaxResultsChange?: (n: number) => void;
		selectedPool: { tribe: string; gender: string | null }[];
		onaddToPool?: (tribe: string) => void;
		onremoveFromPool?: (tribe: string) => void;
		onsetPoolGender?: (tribe: string, gender: string | null) => void;
		players: PlayerSummaryT[];
		ownerUid: string | null;
		onownerUidChange?: (uid: string | null) => void;
		includeWild: boolean;
		onincludeWildChange?: (val: boolean) => void;
		saveLoaded: boolean;
		computing: boolean;
		canRunChain: boolean;
		oncompute?: () => void;
		error: string | null;
		palMap: Map<string, BreedablePal>;
		passiveName?: (asset: string) => string;
		selectedNode: NodeDetail | null;
		pals?: BreedablePal[];
	} = $props();
</script>

<div class="flex flex-col h-full overflow-y-auto">
	<div class="flex items-center justify-between px-3 py-1.5 border-b border-surface-700/20 shrink-0">
		<span
			class="text-[10px] font-semibold text-surface-400 uppercase tracking-wider {collapsed
				? 'hidden'
				: ''}">Controls</span
		>
		<button
			class="btn btn-secondary p-1 rounded-3 text-surface-400 hover:text-surface-50 transition-colors"
			onclick={() => oncollapsedChange?.(!collapsed)}
			title={collapsed ? 'Expand panel' : 'Collapse panel'}
		>
			{#if collapsed}<ChevronLeft size={13} />{:else}<ChevronRight size={13} />{/if}
		</button>
	</div>

	{#if collapsed}
		<div class="flex flex-col items-center gap-2 py-3 text-surface-400">
			<span class="text-[9px] font-medium">Cfg</span>
		</div>
	{:else}
		{#if chains.length > 0}
			<div class="px-3 pt-3 pb-2 border-b border-surface-700/20">
				<span class="block text-[10px] font-semibold text-surface-400 uppercase tracking-wider mb-1.5">
					{m.breeding_chains()} ({chains.length})
				</span>
				<div class="flex flex-wrap gap-1">
					{#each chains as chain, i}
						<button
							class="px-2 py-1 rounded-3 text-[10px] font-medium transition-all {i === activeChainIndex ? 'bg-primary-500/15 text-primary-300 border border-primary-500/40' : 'text-surface-200 hover:bg-surface-800 border border-surface-700/30'}"
							onclick={() => onactiveChainIndexChange?.(i)}
							title="{palMap.get(chain.target)?.display_name ?? chain.target} — {chain.generations} gen"
						>
							{i + 1}
						</button>
					{/each}
				</div>
			</div>
		{/if}

		{#if mode === 'direct'}
			<div class="px-3 py-3 border-b border-surface-700/20 space-y-2.5">
				<span class="block text-[10px] font-semibold text-surface-400 uppercase tracking-wider">Direct</span>
				<div class="flex gap-1">
					{#each ['forward', 'reverse', 'parents'] as sub}
						<button
							class="px-2 py-1 rounded-3 text-[9px] font-medium transition-all {directSub === sub ? 'bg-surface-800 text-surface-50 border border-surface-600/60' : 'text-surface-400 hover:text-surface-200 border border-transparent'}"
							onclick={() => ondirectSubChange?.(sub)}
						>
							{sub === 'forward' ? 'A+B→C' : sub === 'reverse' ? 'A+T→B' : 'T→P'}
						</button>
					{/each}
				</div>

				{#if directSub !== 'parents'}
					<div>
						<span class="block text-[10px] text-surface-400 mb-0.5">Parent A</span>
						<PalPicker pals={pals} value={parentA} onselect={(t) => onparentAChange?.(t)} />
					</div>
				{/if}
				{#if directSub === 'forward'}
					<div>
						<span class="block text-[10px] text-surface-400 mb-0.5">Parent B</span>
						<PalPicker pals={pals} value={parentB} onselect={(t) => onparentBChange?.(t)} exclude={parentA ? [parentA] : []} />
					</div>
				{:else if directSub === 'reverse' || directSub === 'parents'}
					<div>
						<span class="block text-[10px] text-surface-400 mb-0.5">Target</span>
						<PalPicker pals={pals} value={directTarget} onselect={(t) => ondirectTargetChange?.(t)} />
					</div>
				{/if}

				<button
					class="btn btn-primary text-xs w-full flex items-center justify-center gap-1.5"
					disabled={!canRunDirect || directLoading}
					onclick={oncomputeDirect}
				>
					{#if directLoading}<Spinner size="size-3.5" />{:else}<Play size={13} />{/if}
					Compute
				</button>
				{#if error}<p class="text-[10px] text-rose-400">{error}</p>{/if}
			</div>
		{:else}
			<div class="px-3 py-3 border-b border-surface-700/20 space-y-2.5">
				<span class="block text-[10px] font-semibold text-surface-400 uppercase tracking-wider">
					{m.breeding_configuration()}
				</span>
				<div>
					<span class="block text-[10px] text-surface-400 mb-0.5">{m.breeding_target()}</span>
					<PalPicker pals={pals} value={chainTarget} onselect={(t) => onchainTargetChange?.(t)} />
				</div>
				<div>
					<span class="block text-[10px] text-surface-400 mb-0.5">{m.breeding_gender()}</span>
					<select
						class="input text-xs w-full"
						value={chainGender ?? ''}
						onchange={(e) =>
							onchainGenderChange?.((e.currentTarget as HTMLSelectElement).value || null)}
					>
						<option value="">{m.breeding_any_gender()}</option>
						<option value="Male">{m.breeding_male()}</option>
						<option value="Female">{m.breeding_female()}</option>
					</select>
				</div>
				<div class="grid grid-cols-2 gap-2">
					<div>
						<span class="block text-[10px] text-surface-400 mb-0.5">{m.breeding_max_generations()}</span>
						<input
							type="number"
							min="1"
							max="6"
							class="input text-xs w-full"
							value={chainGens}
							oninput={(e) =>
								onchainGensChange?.(parseInt((e.currentTarget as HTMLInputElement).value) || 4)}
						/>
					</div>
					<div>
						<span class="block text-[10px] text-surface-400 mb-0.5">{m.breeding_max_results()}</span>
						<input
							type="number"
							min="1"
							max="10"
							class="input text-xs w-full"
							value={chainMaxResults}
							oninput={(e) =>
								onchainMaxResultsChange?.(parseInt((e.currentTarget as HTMLInputElement).value) || 5)}
						/>
					</div>
				</div>
			</div>

			<div class="px-3 py-3 border-b border-surface-700/20 space-y-2.5">
				{#if mode === 'selection'}
					<div>
						<span class="block text-[10px] font-semibold text-surface-400 uppercase tracking-wider mb-1.5">
							{m.breeding_pool()} ({selectedPool.length})
						</span>
						<PalPicker
							pals={pals}
							placeholder={m.breeding_add_to_pool()}
							onselect={(t) => onaddToPool?.(t)}
							exclude={selectedPool.map((p) => p.tribe)}
						/>
						{#if selectedPool.length}
							<div class="flex flex-wrap gap-1 mt-1.5">
								{#each selectedPool as member}
									<div
										class="flex items-center gap-1 px-1.5 py-0.5 rounded-4 bg-surface-950/50 border border-surface-700/30"
									>
										<PalSlot
											tribe={member.tribe}
											display={palMap.get(member.tribe)?.display_name}
											characterId={member.tribe}
											size="sm"
										/>
										<select
											class="bg-transparent text-[8px] text-surface-400 outline-none cursor-pointer"
											value={member.gender ?? ''}
											onchange={(e) =>
												onsetPoolGender?.(
													member.tribe,
													(e.currentTarget as HTMLSelectElement).value || null
												)}
										>
											<option value="">Any</option>
											<option value="Male">M</option>
											<option value="Female">F</option>
										</select>
										<button
											class="text-surface-400 hover:text-rose-400 transition-colors"
											onclick={() => onremoveFromPool?.(member.tribe)}
											title="Remove"
										>
											<X size={10} />
										</button>
									</div>
								{/each}
							</div>
						{/if}
					</div>
				{:else}
					{#if !saveLoaded}
						<div class="text-xs text-surface-400 italic py-2 text-center">
							{m.breeding_save_required_hint()}
						</div>
					{:else}
						<div class="space-y-2.5">
							<OwnerSelect {players} {ownerUid} onownerUidChange={(uid) => onownerUidChange?.(uid)} />
							<div>
								<label class="flex items-center gap-2 text-xs text-surface-200 cursor-pointer">
									<input
										type="checkbox"
										checked={includeWild}
										onchange={(e) =>
											onincludeWildChange?.((e.currentTarget as HTMLInputElement).checked)}
										class="accent-primary-500"
									/>
									{m.breeding_include_wild()}
								</label>
							</div>
						</div>
					{/if}
				{/if}

				<button
					class="btn btn-primary text-xs w-full flex items-center justify-center gap-1.5"
					disabled={!canRunChain || computing}
					onclick={oncompute}
				>
					{#if computing}<Spinner size="size-3.5" />{:else}{m.breeding_find_chains()}{/if}
				</button>
				{#if error}<p class="text-[10px] text-rose-400">{error}</p>{/if}
			</div>

			{#if selectedNode}
				<div class="px-3 py-3 border-b border-surface-700/20">
					<span class="block text-[10px] font-semibold text-surface-400 uppercase tracking-wider mb-1.5">
						{m.breeding_node_details()}
					</span>
					<div class="space-y-1">
						<p class="text-xs font-medium text-surface-50">{selectedNode.display}</p>
						<p class="text-[10px] text-surface-400 font-mono">{selectedNode.tribe}</p>
						{#if selectedNode.passives.length}
							<div class="flex flex-wrap gap-1">
								{#each selectedNode.passives as p}
									<span class="chip text-[9px] px-1.5 py-0">{passiveName(p)}</span>
								{/each}
							</div>
						{/if}
					</div>
				</div>
			{/if}
		{/if}
	{/if}
</div>
