<script lang="ts">
	import { passiveSkillsData, activeSkillsData } from '$lib/data';
	import * as m from '$i18n/messages';
	import type { OverviewStats } from '$states';

	let { composition }: { composition: OverviewStats['composition'] } = $props();

	const maxBracket = $derived(
		Math.max(1, ...composition.level_brackets.map((bracket) => bracket.count))
	);
	const genderTotal = $derived(
		Math.max(1, composition.gender.male + composition.gender.female + composition.gender.unknown)
	);

	function pct(count: number): string {
		return `${Math.round((count / genderTotal) * 100)}%`;
	}

	function passiveName(skill: string): string {
		return passiveSkillsData.getByKey(skill)?.localized_name ?? skill;
	}

	function activeName(skill: string): string {
		return activeSkillsData.getByKey(skill)?.localized_name ?? skill;
	}
</script>

<div class="card">
	<h3 class="text-surface-400 mb-4 text-xs font-semibold tracking-wider uppercase">
		{m.overview_pal_composition()}
	</h3>

	<!-- 2x2: level | top passives / gender + talents | top actives -->
	<div class="grid grid-cols-1 gap-x-8 gap-y-6 md:grid-cols-2">
		<!-- Level brackets + average -->
		<div class="flex flex-col gap-3">
			<div class="flex items-baseline justify-between">
				<h4 class="text-surface-500 text-sm font-medium">{m.level()}</h4>
				<span class="text-surface-100 text-xl font-bold tabular-nums">
					{composition.avg_level.toFixed(1)}
				</span>
			</div>
			<p class="text-surface-500 text-xs">{m.avg_level()}</p>
			{#each composition.level_brackets as bracket (bracket.label)}
				<div class="flex items-center gap-2">
					<span class="text-surface-500 w-12 shrink-0 text-xs tabular-nums">
						{bracket.label}
					</span>
					<div class="bg-surface-900/60 h-2 flex-1 overflow-hidden rounded-full">
						<div
							class="bg-primary-500 h-full rounded-full transition-all"
							style="width: {Math.round((bracket.count / maxBracket) * 100)}%"
						></div>
					</div>
					<span class="text-surface-300 w-10 shrink-0 text-right text-xs tabular-nums">
						{bracket.count.toLocaleString()}
					</span>
				</div>
			{/each}
		</div>

		<!-- Top passives -->
		<div class="flex min-w-0 flex-col gap-2">
			<h4 class="text-surface-500 text-sm font-medium">{m.overview_top_passives()}</h4>
			<div class="flex flex-wrap gap-1.5">
				{#each composition.top_passives as entry (entry.skill)}
					<span
						class="border-primary-500/40 bg-primary-500/10 text-primary-300 truncate rounded-full border px-2.5 py-0.5 text-xs font-medium"
						title="{passiveName(entry.skill)} ×{entry.count}"
					>
						{passiveName(entry.skill)} ×{entry.count}
					</span>
				{:else}
					<span class="text-surface-500 text-xs">—</span>
				{/each}
			</div>
		</div>

		<!-- Gender split + talent averages -->
		<div class="flex flex-col gap-3">
			<h4 class="text-surface-500 text-sm font-medium">{m.gender()}</h4>
			<div class="flex h-3 overflow-hidden rounded-full" role="img" aria-label={m.gender()}>
				<div class="h-full bg-blue-500" style="width: {pct(composition.gender.male)}"></div>
				<div class="h-full bg-pink-500" style="width: {pct(composition.gender.female)}"></div>
				<div class="bg-surface-600 h-full" style="width: {pct(composition.gender.unknown)}"></div>
			</div>
			<div class="text-surface-300 grid grid-cols-3 gap-1 text-center text-xs">
				<div>
					<span class="text-blue-400">●</span>
					{m.overview_male()}
					<span class="block font-medium tabular-nums">
						{composition.gender.male.toLocaleString()}
					</span>
				</div>
				<div>
					<span class="text-pink-400">●</span>
					{m.overview_female()}
					<span class="block font-medium tabular-nums">
						{composition.gender.female.toLocaleString()}
					</span>
				</div>
				<div>
					<span class="text-surface-500">●</span>
					{m.unknown()}
					<span class="block font-medium tabular-nums">
						{composition.gender.unknown.toLocaleString()}
					</span>
				</div>
			</div>

			<h4 class="text-surface-500 mt-3 text-sm font-medium">
				{m.overview_talent_averages()}
			</h4>
			<div class="text-surface-100 grid grid-cols-3 gap-1 text-center">
				<div>
					<span class="block text-lg font-bold tabular-nums">
						{composition.talent_avg.hp.toFixed(1)}
					</span>
					<span class="text-surface-500 text-xs">{m.hp()}</span>
				</div>
				<div>
					<span class="block text-lg font-bold tabular-nums">
						{composition.talent_avg.attack.toFixed(1)}
					</span>
					<span class="text-surface-500 text-xs">{m.attack()}</span>
				</div>
				<div>
					<span class="block text-lg font-bold tabular-nums">
						{composition.talent_avg.defense.toFixed(1)}
					</span>
					<span class="text-surface-500 text-xs">{m.defense()}</span>
				</div>
			</div>
		</div>

		<!-- Top active skills -->
		<div class="flex min-w-0 flex-col gap-2">
			<h4 class="text-surface-500 text-sm font-medium">{m.overview_top_actives()}</h4>
			<div class="flex flex-wrap gap-1.5">
				{#each composition.top_actives as entry (entry.skill)}
					<span
						class="border-secondary-500/40 bg-secondary-500/10 text-secondary-300 truncate rounded-full border px-2.5 py-0.5 text-xs font-medium"
						title="{activeName(entry.skill)} ×{entry.count}"
					>
						{activeName(entry.skill)} ×{entry.count}
					</span>
				{:else}
					<span class="text-surface-500 text-xs">—</span>
				{/each}
			</div>
		</div>
	</div>
</div>
