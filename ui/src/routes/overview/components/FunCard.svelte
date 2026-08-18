<script lang="ts">
	import { palsData } from '$lib/data';
	import { cn } from '$theme';
	import Dices from '@lucide/svelte/icons/dices';
	import PartyPopper from '@lucide/svelte/icons/party-popper';
	import type { OverviewStats } from '$states';
	import * as m from '$i18n/messages';

	let { stats }: { stats: OverviewStats } = $props();

	// Bump to re-roll which three facts are on display.
	let roll = $state(0);

	function fmt(n: number): string {
		return n.toLocaleString();
	}

	/** Top species by headcount, localized, or null when the world is empty. */
	function topSpecies(): { name: string; count: number } | null {
		const entry = stats.top_species[0];
		if (!entry) return null;
		return {
			name: palsData.getByKey(entry.key)?.localized_name ?? entry.key,
			count: entry.count
		};
	}

	function topPlayer(): { name: string; level: number | null; pals: number } | null {
		const player = stats.top_players[0];
		if (!player) return null;
		return {
			name: player.nickname || '(someone)',
			level: player.level,
			pals: player.pal_count
		};
	}

	/** All the light-hearted "server facts", built from the real numbers. */
	function buildFacts(): string[] {
		const { totals, traits, condition, composition } = stats;
		const pool: string[] = [];

		const combinedLevel = Math.round(totals.pals * composition.avg_level);
		if (totals.pals > 0 && composition.avg_level > 0) {
			// Combined estimated level: every pal's level summed up.
			pool.push(
				`Combined, all ${fmt(totals.pals)} pals hold roughly level ${fmt(combinedLevel)}. That's about ${fmt(
					Math.round(combinedLevel / 60)
				)} max-level pals stacked on each other's shoulders.`
			);
		}

		if (traits.rare_pals > 0) {
			pool.push(
				`${fmt(traits.rare_pals)} lucky pals on the server? Boy, people sure are lucky around here. Buy a lottery ticket already.`
			);
		} else {
			pool.push('Zero lucky pals spotted. The grind continues, comrade.');
		}

		if (traits.boss_pals > 0) {
			pool.push(
				`${fmt(traits.boss_pals)} alpha bosses roaming free. Who let them out, and more importantly — who is paying for the property damage?`
			);
		}

		if (traits.awakened_pals > 0) {
			pool.push(
				`${fmt(traits.awakened_pals)} awakened pals flexing on everyone else. Very humble crowd.`
			);
		}

		const genderTotal =
			composition.gender.male + composition.gender.female + composition.gender.unknown;
		if (genderTotal > 0) {
			const malePct = Math.round((composition.gender.male / genderTotal) * 100);
			const femalePct = Math.round((composition.gender.female / genderTotal) * 100);
			pool.push(
				`Server gender ratio: ${malePct}% male, ${femalePct}% female, and a whole ${fmt(
					composition.gender.unknown
				)} pals keeping everyone guessing.`
			);
		}

		if (condition.sick_pals > 0) {
			pool.push(
				`${fmt(condition.sick_pals)} sick pals. Someone clearly skipped the vitamin-berry aisle at the Pal merchant.`
			);
		}
		if (condition.fainted_pals > 0) {
			pool.push(
				`${fmt(condition.fainted_pals)} fainted pals. Did the server survive a raid, or a very aggressive picnic?`
			);
		}

		const species = topSpecies();
		if (species) {
			pool.push(
				`The unofficial server mascot is ${species.name} with ${fmt(
					species.count
				)} sightings. Move over, everyone else.`
			);
			if (stats.top_species[1]) {
				const runnerUp =
					palsData.getByKey(stats.top_species[1].key)?.localized_name ?? stats.top_species[1].key;
				pool.push(
					`Runner-up: ${runnerUp} with ${fmt(stats.top_species[1].count)}. Solid try, ${runnerUp}.`
				);
			}
		}

		const player = topPlayer();
		if (player) {
			if (player.level != null && player.pals > 0) {
				pool.push(
					`Oi, ${player.name} — level ${player.level}, ${fmt(
						player.pals
					)} pals in tow. Pretty strong, ya know. Don't let it go to your head.`
				);
			} else {
				pool.push(`Oi, ${player.name} — nice save. Very impressive. The palbox thinks so too.`);
			}
		}

		if (totals.species > 0) {
			pool.push(
				`${fmt(totals.species)} species in one world. The Paldeck is basically complete — flex on your friends.`
			);
		}

		if (totals.human_npcs > 0) {
			pool.push(
				`${fmt(totals.human_npcs)} human NPCs employed. The staffing agency is thriving; HR is not.`
			);
		}

		if (totals.guilds > 0) {
			pool.push(
				`${fmt(totals.guilds)} guilds on the server, and they still can't coordinate a raid schedule. Classic.`
			);
		}

		const bracket = composition.level_brackets.reduce((biggest, current) =>
			current.count > biggest.count ? current : biggest
		);
		if (bracket && bracket.count > 0) {
			pool.push(
				`Most pals hang out in the ${bracket.label} range (${fmt(
					bracket.count
				)} of them). Nobody wants to grind past 60.`
			);
		}

		if (traits.boss_pals > 0 && traits.rare_pals > 0) {
			pool.push(
				`${fmt(traits.boss_pals)} alphas AND ${fmt(
					traits.rare_pals
				)} luckies in one world? The server is a walking show-off contest.`
			);
		}

		if (condition.sick_pals === 0 && condition.fainted_pals === 0) {
			pool.push('No sick or fainted pals. Everyone is thriving. Suspiciously thriving.');
		}

		if (totals.containers > 0) {
			pool.push(
				`${fmt(totals.containers)} containers on record. Hoarders Anonymous is meeting at the Palbox, bring your own storage.`
			);
		}

		// Light meta wink at the Overview Full view this card rides along with.
		pool.push(
			'This certified scientific report was produced by the Overview Full Mode research division. Yes, it is that serious.'
		);

		return pool;
	}

	const facts = $derived(buildFacts());

	// Show three (or fewer) facts; each roll rotates the window by one.
	const visible = $derived.by(() => {
		if (facts.length === 0) return [];
		const rotated = [...facts.slice(roll % facts.length), ...facts.slice(0, roll % facts.length)];
		return rotated.slice(0, Math.min(3, facts.length));
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
			onclick={() => (roll += 1)}
		>
			<Dices size={14} />
			{m.overview_refresh()}
		</button>
	</div>

	<div class="flex flex-col gap-3">
		{#each visible as fact (fact)}
			<div
				class={cn(
					'bg-surface-900/40 border-surface-700/50 text-surface-200 rounded-md border px-4 py-3 text-sm leading-relaxed'
				)}
			>
				<span class="text-secondary-400 mr-2 select-none">♪</span>
				{fact}
			</div>
		{/each}
	</div>
</div>
