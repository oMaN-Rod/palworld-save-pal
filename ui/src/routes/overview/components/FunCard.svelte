<script lang="ts">
	import { activeSkillsData, palsData, passiveSkillsData } from '$lib/data';
	import { cn } from '$theme';
	import Dices from '@lucide/svelte/icons/dices';
	import Globe2 from '@lucide/svelte/icons/globe-2';
	import PartyPopper from '@lucide/svelte/icons/party-popper';
	import Sparkles from '@lucide/svelte/icons/sparkles';
	import type { OverviewStats } from '$states';
	import * as m from '$i18n/messages';
	import {
		computeWorldLevel,
		generateShenanigans,
		type Shenanigan,
		type ShenaniganKind
	} from '$lib/utils/serverShenanigans';
	import { untrack } from 'svelte';

	let { stats }: { stats: OverviewStats } = $props();

	/** Narrator voices — the "AI" rotates between them so rolls feel fresh. */
	const KIND_META: Record<ShenaniganKind, { label: string; chip: string }> = {
		science: {
			label: 'Certified Science',
			chip: 'border-primary-500/40 bg-primary-500/10 text-primary-300'
		},
		roast: { label: 'Roast', chip: 'border-error-500/40 bg-error-500/10 text-error-300' },
		conspiracy: {
			label: 'Conspiracy',
			chip: 'border-warning-500/40 bg-warning-500/10 text-warning-300'
		},
		prophecy: {
			label: 'Prophecy',
			chip: 'border-secondary-500/40 bg-secondary-500/10 text-secondary-300'
		},
		hr: { label: 'HR Memo', chip: 'border-tertiary-500/40 bg-tertiary-500/10 text-tertiary-300' },
		nature: {
			label: 'Nature Doc',
			chip: 'border-success-500/40 bg-success-500/10 text-success-300'
		},
		weather: { label: 'Weather', chip: 'border-blue-500/40 bg-blue-500/10 text-blue-300' },
		commentary: {
			label: 'Commentary',
			chip: 'border-orange-500/40 bg-orange-500/10 text-orange-300'
		}
	};

	const world = $derived(computeWorldLevel(stats));

	const names = {
		passive: (key: string) => passiveSkillsData.getByKey(key)?.localized_name ?? key,
		active: (key: string) => activeSkillsData.getByKey(key)?.localized_name ?? key,
		species: (key: string) => palsData.getByKey(key)?.localized_name ?? key
	};

	// The current narrator lineup; each roll deals a fresh hand that avoids
	// whatever is already on screen.
	let current = $state<Shenanigan[]>([]);

	function reroll(avoid: string[] = []) {
		current = generateShenanigans(stats, names, { count: 3, avoid });
	}

	// Re-deal whenever the stats object changes (new save / refresh) or the
	// skill catalogs finish loading mid-session. `current` is only read
	// untracked, so writing it here cannot re-trigger the effect.
	$effect(() => {
		void stats;
		void passiveSkillsData.passiveSkills;
		void activeSkillsData.activeSkills;
		void palsData.pals;
		reroll(untrack(() => current.map((fact) => fact.text)));
	});
</script>

<div class="card">
	<div class="mb-4 flex items-center justify-between gap-3">
		<h3 class="flex items-center gap-2 text-xs font-semibold tracking-wider uppercase">
			<PartyPopper size={16} class="text-warning-400" />
			<span class="text-surface-400">Server Shenanigans</span>
		</h3>
		<button
			type="button"
			class="border-surface-600/60 text-surface-300 hover:border-primary-400/60 hover:text-surface-100 flex items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-xs font-medium transition-colors"
			onclick={() => reroll(current.map((fact) => fact.text))}
		>
			<Dices size={14} />
			{m.overview_refresh()}
		</button>
	</div>

	<!-- The Official* Server Level -->
	<div
		class="bg-surface-900/40 border-surface-700/50 rounded-md border px-4 py-4"
		title={world.formula}
	>
		<div class="flex items-center justify-between gap-3">
			<div class="flex min-w-0 flex-col gap-0.5">
				<span class="text-surface-500 text-[10px] font-semibold tracking-wider uppercase">
					Official* Server Level
				</span>
				<span class="heading-gradient text-4xl leading-tight font-extrabold tabular-nums">
					{world.level.toLocaleString()}
				</span>
				<span class="text-secondary-300 truncate text-xs font-medium">
					{world.tier} — {world.tierBlurb}
				</span>
			</div>
			<div class="flex shrink-0 flex-col items-center gap-1">
				{#if world.over9000}
					<Sparkles size={36} class="text-warning-400" />
					<span class="text-warning-300 text-[10px] font-bold">MAX</span>
				{:else}
					<Globe2 size={36} class="text-primary-400/70" />
				{/if}
			</div>
		</div>
		<p class="text-surface-300 mt-3 text-sm leading-relaxed">{world.headline}</p>
		<p class="text-surface-600 mt-1.5 text-[10px] leading-relaxed">
			*Certified by the Overview Full Mode research division. Formula on hover. Peer-reviewed by a
			Lamball.
		</p>
	</div>

	<!-- The rotating narrator lineup -->
	<div class="mt-3 flex flex-col gap-3">
		{#each current as fact (fact.text)}
			<div
				class="bg-surface-900/40 border-surface-700/50 rounded-md border px-4 py-3 text-sm leading-relaxed"
			>
				<div class="mb-1.5 flex items-center gap-2">
					<span
						class={cn(
							'rounded-full border px-2 py-0.5 text-[9px] font-bold tracking-wider uppercase',
							KIND_META[fact.kind].chip
						)}
					>
						{KIND_META[fact.kind].label}
					</span>
				</div>
				<p class="text-surface-200">{fact.text}</p>
			</div>
		{/each}
	</div>
</div>
