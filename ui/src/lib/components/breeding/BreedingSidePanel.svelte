<script lang="ts">
	/**
	 * BreedingSidePanel — right-hand panel for Graph Mode. Contains the chain
	 * selector tabs, configuration inputs, selection-pool or save-owner
	 * controls, the compute button, and a node-detail section.
	 */
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
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

<div class="flex h-full flex-col overflow-y-auto">
	<div
		class="border-surface-700/20 flex shrink-0 items-center justify-between border-b px-3 py-1.5 {collapsed
			? 'hidden'
			: ''}"
	>
		<span class="text-surface-400 text-xs font-semibold tracking-wider uppercase">{m.breeding_controls()}</span>
		<button
			class="btn btn-secondary text-surface-400 hover:text-surface-50 rounded-sm p-1 transition-colors"
			onclick={() => oncollapsedChange?.(!collapsed)}
			title={m.breeding_collapse_panel()}
		>
			<ChevronRight size={13} />
		</button>
	</div>

	{#if collapsed}
		<button
			class="text-surface-400 hover:text-surface-100 flex w-full flex-1 flex-col items-center justify-center gap-3 py-3 transition-colors"
			onclick={() => oncollapsedChange?.(!collapsed)}
			title={m.breeding_expand_panel()}
		>
			<ChevronLeft size={14} />
			<span class="text-[10px] font-medium tracking-widest uppercase [writing-mode:vertical-rl]">
				{m.breeding_cfg()}
			</span>
		</button>
	{:else}
		{#if chains.length > 0}
			<div class="border-surface-700/20 border-b px-3 pt-3 pb-2">
				<span class="text-surface-400 mb-1.5 block text-xs font-semibold tracking-wider uppercase">
					{m.breeding_chains()} ({chains.length})
				</span>
				<div class="flex flex-wrap gap-1">
					{#each chains as chain, i}
						<button
							class="rounded-sm px-2 py-1 text-xs font-medium transition-all {i === activeChainIndex
								? 'bg-primary-500/15 text-primary-300 border-primary-500/40 border'
								: 'text-surface-200 hover:bg-surface-800 border-surface-700/30 border'}"
							onclick={() => onactiveChainIndexChange?.(i)}
							title={m.breeding_gen_title({
								name: palMap.get(chain.target)?.display_name ?? chain.target,
								n: chain.generations
							})}
						>
							{i + 1}
						</button>
					{/each}
				</div>
			</div>
		{/if}

		{#if mode === 'direct'}
			<div class="border-surface-700/20 space-y-2.5 border-b px-3 py-3">
				<span class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
					>{m.breeding_direct()}</span
				>
				<div class="flex gap-1">
					{#each ['forward', 'reverse', 'parents'] as sub}
						<button
							class="rounded-sm px-2 py-1 text-xs font-medium transition-all {directSub === sub
								? 'bg-surface-800 text-surface-50 border-surface-600/60 border'
								: 'text-surface-400 hover:text-surface-200 border border-transparent'}"
							onclick={() => ondirectSubChange?.(sub)}
						>
							{sub === 'forward'
								? m.breeding_a_plus_b_child()
								: sub === 'reverse'
									? m.breeding_a_plus_target()
									: m.breeding_target_with_parents()}
						</button>
					{/each}
				</div>

				{#if directSub !== 'parents'}
					<div>
						<span class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
							>{m.breeding_parent_a()}</span
						>
						<PalPicker {pals} value={parentA} onselect={(t) => onparentAChange?.(t)} />
					</div>
				{/if}
				{#if directSub === 'forward'}
					<div>
						<span class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
							>{m.breeding_parent_b()}</span
						>
						<PalPicker
							{pals}
							value={parentB}
							onselect={(t) => onparentBChange?.(t)}
							exclude={parentA ? [parentA] : []}
						/>
					</div>
				{:else if directSub === 'reverse' || directSub === 'parents'}
					<div>
						<span class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
							>{m.breeding_target()}</span
						>
						<PalPicker {pals} value={directTarget} onselect={(t) => ondirectTargetChange?.(t)} />
					</div>
				{/if}

				<button
					class="btn btn-primary flex w-full items-center justify-center gap-1.5 text-xs"
					disabled={!canRunDirect || directLoading}
					onclick={oncomputeDirect}
				>
					{#if directLoading}<Spinner size="size-3.5" />{:else}<Play size={13} />{/if}
					{m.breeding_compute()}
				</button>
				{#if error}<p class="text-error-400 text-xs">{error}</p>{/if}
			</div>
		{:else}
			<div class="border-surface-700/20 space-y-2.5 border-b px-3 py-3">
				<span class="text-surface-400 block text-xs font-semibold tracking-wider uppercase">
					{m.breeding_configuration()}
				</span>
				<div>
					<span class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
						>{m.breeding_target()}</span
					>
					<PalPicker {pals} value={chainTarget} onselect={(t) => onchainTargetChange?.(t)} />
				</div>
				<div>
					<span class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
						>{m.breeding_gender()}</span
					>
					<select
						class="input w-full text-xs"
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
						<span class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
							>{m.breeding_max_generations()}</span
						>
						<input
							type="number"
							min="1"
							max="6"
							class="input w-full text-xs"
							value={chainGens}
							oninput={(e) =>
								onchainGensChange?.(parseInt((e.currentTarget as HTMLInputElement).value) || 4)}
						/>
					</div>
					<div>
						<span class="text-surface-400 mb-1 block text-xs font-semibold tracking-wider uppercase"
							>{m.breeding_max_results()}</span
						>
						<input
							type="number"
							min="1"
							max="10"
							class="input w-full text-xs"
							value={chainMaxResults}
							oninput={(e) =>
								onchainMaxResultsChange?.(
									parseInt((e.currentTarget as HTMLInputElement).value) || 5
								)}
						/>
					</div>
				</div>
			</div>

			<div class="border-surface-700/20 space-y-2.5 border-b px-3 py-3">
				{#if mode === 'selection'}
					<div>
						<span
							class="text-surface-400 mb-1.5 block text-xs font-semibold tracking-wider uppercase"
						>
							{m.breeding_pool()} ({selectedPool.length})
						</span>
						<PalPicker
							{pals}
							placeholder={m.breeding_add_to_pool()}
							onselect={(t) => onaddToPool?.(t)}
							exclude={selectedPool.map((p) => p.tribe)}
						/>
						{#if selectedPool.length}
							<div class="mt-1.5 flex flex-wrap gap-1">
								{#each selectedPool as member}
									<div
										class="bg-surface-950/50 border-surface-700/30 flex items-center gap-1 rounded-sm border px-1.5 py-0.5"
									>
										<PalSlot
											tribe={member.tribe}
											display={palMap.get(member.tribe)?.display_name}
											characterId={member.tribe}
											size="sm"
										/>
										<select
											class="text-surface-400 cursor-pointer bg-transparent text-[10px] outline-none"
											value={member.gender ?? ''}
											onchange={(e) =>
												onsetPoolGender?.(
													member.tribe,
													(e.currentTarget as HTMLSelectElement).value || null
												)}
										>
											<option value="">{m.breeding_any()}</option>
											<option value="Male">{m.breeding_male_short()}</option>
											<option value="Female">{m.breeding_female_short()}</option>
										</select>
										<button
											class="text-surface-400 hover:text-error-400 transition-colors"
											onclick={() => onremoveFromPool?.(member.tribe)}
											title={m.breeding_remove()}
										>
											<X size={10} />
										</button>
									</div>
								{/each}
							</div>
						{/if}
					</div>
				{:else if !saveLoaded}
					<div class="text-surface-400 py-2 text-center text-xs italic">
						{m.breeding_save_required_hint()}
					</div>
				{:else}
					<div class="space-y-2.5">
						<OwnerSelect {players} {ownerUid} onownerUidChange={(uid) => onownerUidChange?.(uid)} />
						<div>
							<label class="text-surface-200 flex cursor-pointer items-center gap-2 text-xs">
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

				<button
					class="btn btn-primary flex w-full items-center justify-center gap-1.5 text-xs"
					disabled={!canRunChain || computing}
					onclick={oncompute}
				>
					{#if computing}<Spinner size="size-3.5" />{:else}{m.breeding_find_chains()}{/if}
				</button>
				{#if error}<p class="text-error-400 text-xs">{error}</p>{/if}
			</div>

			{#if selectedNode}
				<div class="border-surface-700/20 border-b px-3 py-3">
					<span
						class="text-surface-400 mb-1.5 block text-xs font-semibold tracking-wider uppercase"
					>
						{m.breeding_node_details()}
					</span>
					<div class="space-y-1">
						<p class="text-surface-50 text-xs font-medium">{selectedNode.display}</p>
						{#if selectedNode.passives.length}
							<div class="flex flex-wrap gap-1">
								{#each selectedNode.passives as p}
									<span class="chip px-1.5 py-0 text-[10px]">{passiveName(p)}</span>
								{/each}
							</div>
						{/if}
					</div>
				</div>
			{/if}
		{/if}
	{/if}
</div>
