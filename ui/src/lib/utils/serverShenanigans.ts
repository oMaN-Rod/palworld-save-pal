/**
 * The Server Shenanigans engine: a stat-driven joke/roast generator for the
 * overview's FunCard. Everything here is deliberately silly — the numbers are
 * real (straight from `get_overview_stats`), the commentary is not.
 *
 * The module is pure TypeScript: skill/species names are resolved through the
 * injected `SkillNames` so tests can stub localization without svelte stores.
 */

import type { OverviewStats } from '$states';

/** Narrator voices a shenanigan can be delivered in. Styling lives in FunCard. */
export type ShenaniganKind =
	| 'science'
	| 'roast'
	| 'conspiracy'
	| 'prophecy'
	| 'hr'
	| 'nature'
	| 'weather'
	| 'commentary';

export interface Shenanigan {
	kind: ShenaniganKind;
	text: string;
}

/** Localized display names for the raw catalog keys the wire stats carry. */
export interface SkillNames {
	passive(key: string): string;
	active(key: string): string;
	species(key: string): string;
}

export interface WorldLevelReport {
	/** The certified(tm) Server Level — raw power divided by science. */
	level: number;
	/** Unrounded weighted power: player levels x2, pal levels, trait bonuses. */
	rawPower: number;
	/** Max-level-pal equivalents — "how many level-60s in a trench coat". */
	palEquivalents: number;
	tier: string;
	tierBlurb: string;
	over9000: boolean;
	/** Human-readable pseudo-formula, shown on hover. */
	formula: string;
	headline: string;
}

const fmt = (n: number): string => Math.round(n).toLocaleString();

function pick<T>(variants: T[]): T {
	return variants[Math.floor(Math.random() * variants.length)] ?? variants[0];
}

// ─────────────────────────────────────────────────────────────────────────────
// The Official* Server Level
// ─────────────────────────────────────────────────────────────────────────────

const TIERS: { min: number; tier: string; blurb: string }[] = [
	{ min: 1200, tier: 'Palpagos Endgame', blurb: 'Certified touch-grass-resistant.' },
	{ min: 600, tier: 'Jetragon Airspace', blurb: 'Please keep limbs inside the ride.' },
	{ min: 300, tier: 'Alpha Territory', blurb: 'The bosses have noticed you.' },
	{ min: 150, tier: 'Lamball Labour Union', blurb: 'Collectively fluffy. Collectively armed.' },
	{ min: 50, tier: 'Cattiva Committee', blurb: 'Governance: chaotic. Morale: weirdly high.' },
	{ min: 1, tier: 'Chikipin Daycare', blurb: 'Population: mostly snacks.' },
	{ min: 0, tier: 'Fresh Save Energy', blurb: 'The islands are basically still unwrapped.' }
];

export function computeWorldLevel(stats: OverviewStats): WorldLevelReport {
	const { totals, traits, composition, top_players } = stats;

	// Only the leaderboard previews carry levels, so "every player" is really
	// "every famous player". They count double — players are built different.
	const playerLevelSum = top_players.reduce((sum, player) => sum + (player.level ?? 0), 0);
	const palLevelSum = totals.pals * composition.avg_level;

	const rawPower =
		playerLevelSum * 2 +
		palLevelSum +
		traits.boss_pals * 5 + // alphas are heavy
		traits.rare_pals * 7 + // luck has mass
		traits.awakened_pals * 3; // awakened energy is a real thing, probably

	const level = Math.round(rawPower / 100);
	const tier = TIERS.find((t) => level >= t.min) ?? TIERS[TIERS.length - 1];
	const over9000 = rawPower > 9000;

	return {
		level,
		rawPower: Math.round(rawPower),
		palEquivalents: Math.round(rawPower / 60),
		tier: tier.tier,
		tierBlurb: tier.blurb,
		over9000,
		formula:
			'(every player level ×2) + (every pal level) + alphas ×5 + luckies ×7 + awakened ×3, ' +
			`then ÷100 because science said so. Current inputs: ${fmt(playerLevelSum)} famous player ` +
			`levels, ${fmt(palLevelSum)} pal levels, ${fmt(traits.boss_pals)} alphas, ` +
			`${fmt(traits.rare_pals)} luckies, ${fmt(traits.awakened_pals)} awakened.`,
		headline: over9000
			? `Raw power: ${fmt(rawPower)}. IT'S OVER 9,000! There's no way that can be right.`
			: `Statistically speaking: ${fmt(rawPower / 60)} max-level pals in a trench coat.`
	};
}

// ─────────────────────────────────────────────────────────────────────────────
// Meme skill callouts — only fire when the actual catalog key tops a chart,
// so the "AI" sounds like it knows this specific server.
// ─────────────────────────────────────────────────────────────────────────────

const MEME_PASSIVES: Record<string, (count: number) => Shenanigan> = {
	// Musclehead
	Noukin: (count) => ({
		kind: 'roast',
		text: pick([
			`Musclehead sweeps the passive charts (${fmt(count)} carriers). Head empty. Pal fast.`,
			`${fmt(count)} Musclehead pals and not one thought behind those eyes. Beautiful, in a way.`
		])
	}),
	// Work Slave
	PAL_CorporateSlave: (count) => ({
		kind: 'hr',
		text: pick([
			`${fmt(count)} pals running Work Slave. HR has stopped answering calls and started updating its résumé.`,
			`Work Slave on ${fmt(count)} pals? This is a server, not a startup. Coffee is not compensation.`
		])
	}),
	// Glutton
	PAL_FullStomach_Up_1: (count) => ({
		kind: 'roast',
		text: `${fmt(count)} Gluttons on record. The communal fridge now has a lawyer.`
	}),
	// Slacker
	CraftSpeed_down2: (count) => ({
		kind: 'hr',
		text: `Top office ailment: Slacker (${fmt(count)} confirmed cases). The base runs on two overworked Chikipins and spite.`
	}),
	// Pacifist
	PAL_ALLAttack_down2: (count) => ({
		kind: 'commentary',
		text: `${fmt(count)} Pacifists enrolled in a game about capturing creatures to fight other creatures. Bold. Confusing. Valid.`
	}),
	// Swift
	MoveSpeed_up_3: (count) => ({
		kind: 'commentary',
		text: `Swift leads the passives (${fmt(count)} carriers) — a server that is late for everything but arriving at speed.`
	}),
	// Legend
	Legend: (count) => ({
		kind: 'science',
		text: `A genuine Legend passive is just walking around on ${fmt(count)} pals like it's normal. The economy of cool is in shambles.`
	}),
	// Conceited
	PAL_conceited: (count) => ({
		kind: 'roast',
		text: `${fmt(count)} Conceited pals. Zero of them have earned it. All of them believe otherwise.`
	}),
	// Hooligan
	PAL_rude: (count) => ({
		kind: 'roast',
		text: `${fmt(count)} Hooligans registered. The Palpagos neighborhood watch sends its regards, and a bill.`
	}),
	// Workaholic
	PAL_Sanity_Down_2: (count) => ({
		kind: 'hr',
		text: `${fmt(count)} Workaholics detected. SAN check incoming. Please ration your sanity responsibly.`
	}),
	// Serious
	CraftSpeed_up1: (count) => ({
		kind: 'science',
		text: `The #1 passive is Serious (${fmt(count)} carriers). Efficiency is up. Fun is down. The spreadsheet grows strong.`
	})
};

const MEME_ACTIVES: Record<string, (count: number, name: string) => Shenanigan> = {
	// Air Cannon
	'EPalWazaID::AirCanon': (count) => ({
		kind: 'roast',
		text: pick([
			`The most owned skill on the server is Air Cannon (${fmt(count)} of them). The deadliest weapon here is, statistically, a sneeze.`,
			`Air Cannon leads with ${fmt(count)} owners. Nobody chose this. Everyone has it. That's the joke.`
		])
	}),
	// Pal Blast
	'EPalWazaID::HyperBeam': (count, name) => ({
		kind: 'commentary',
		text: `When in doubt, ${fmt(count)} pals simply shout very loudly ("${name}"). It's a lifestyle.`
	}),
	// Seed Machine Gun
	'EPalWazaID::SeedMachinegun': (count, name) => ({
		kind: 'commentary',
		text: `"${name}" ×${fmt(count)} — because why say it once when a machine gun says it forty times.`
	})
};

const DRAMATIC_ACTIVES = new Set([
	'EPalWazaID::ShadowBall',
	'EPalWazaID::DarkLaser',
	'EPalWazaID::GhostFlame',
	'EPalWazaID::IcicleThrow',
	'EPalWazaID::LightningStrike'
]);

// ─────────────────────────────────────────────────────────────────────────────
// The generator roster
// ─────────────────────────────────────────────────────────────────────────────

type Generator = () => Shenanigan | null;

function buildGenerators(stats: OverviewStats, names: SkillNames): Generator[] {
	const { totals, traits, condition, composition } = stats;
	const topPassive = composition.top_passives[0];
	const secondPassive = composition.top_passives[1];
	const topActive = composition.top_actives[0];
	const mascot = stats.top_species[0];
	const runnerUp = stats.top_species[1];
	const top = stats.top_players[0];
	const second = stats.top_players[1];
	const biggestBracket = composition.level_brackets.reduce(
		(biggest, current) => (current.count > biggest.count ? current : biggest),
		composition.level_brackets[0]
	);
	const palsPerPlayer = totals.players > 0 ? totals.pals / totals.players : 0;
	const mascotShare = totals.pals > 0 && mascot ? mascot.count / totals.pals : 0;
	const passiveName = (key: string) => names.passive(key);
	const activeName = (key: string) => names.active(key);

	return [
		// ── Trait chaos ──
		() =>
			traits.rare_pals > 0
				? {
						kind: pick(['roast', 'conspiracy'] as const),
						text: pick([
							`${fmt(traits.rare_pals)} lucky pals on this server? Boy, people sure are lucky around here. Buy a lottery ticket already.`,
							`${fmt(traits.rare_pals)} lucky pals and counting. The island casino has been notified. It is not happy.`,
							`${fmt(traits.rare_pals)} lucky pals caught and not one apology to the odds. Shameless. Iconic.`
						])
					}
				: {
						kind: 'commentary',
						text: pick([
							'Zero lucky pals spotted. The grind continues, comrade.',
							'Not a single lucky pal on record. The universe is saving them all for one very smug future player.'
						])
					},
		() =>
			traits.boss_pals > 0
				? {
						kind: 'roast',
						text:
							composition.avg_level < 20
								? `${fmt(traits.boss_pals)} alphas and the average pal level is ${composition.avg_level.toFixed(1)}. That is not a server, that is a daycare with security issues.`
								: pick([
										`${fmt(traits.boss_pals)} alpha bosses roaming free. Who let them out, and more importantly — who is paying for the property damage?`,
										`${fmt(traits.boss_pals)} alphas at large. Tourists: do not feed them. Residents: do not be the food.`
									])
					}
				: null,
		() =>
			traits.awakened_pals > 0
				? {
						kind: 'science',
						text: pick([
							`${fmt(traits.awakened_pals)} awakened pals flexing on everyone else. Very humble crowd.`,
							`${fmt(traits.awakened_pals)} awakened pals. The rest of the server is pretending not to notice. They notice.`
						])
					}
				: null,
		() =>
			traits.boss_pals > 0 && traits.rare_pals > 0 && traits.boss_pals + traits.rare_pals > 10
				? {
						kind: 'roast',
						text: `${fmt(traits.boss_pals)} alphas AND ${fmt(traits.rare_pals)} luckies in one world? The server is a walking show-off contest.`
					}
				: null,

		// ── Passives / personality diagnosis ──
		() => {
			if (!topPassive) return null;
			const meme = MEME_PASSIVES[topPassive.skill];
			if (meme) return meme(topPassive.count);
			return {
				kind: 'science',
				text: pick([
					`The server-wide personality is officially "${passiveName(topPassive.skill)}" (${fmt(topPassive.count)} carriers). Diagnosis: correctable, but why bother.`,
					`${fmt(topPassive.count)} pals share the "${passiveName(topPassive.skill)}" passive. That is not a meta, that is a personality cult.`,
					`Server therapy summary: ${fmt(topPassive.count)} cases of "${passiveName(topPassive.skill)}". Prognosis: adorable.`
				])
			};
		},
		() =>
			topPassive && totals.pals > 0 && topPassive.count / totals.pals >= 0.2
				? {
						kind: 'roast',
						text: pick([
							`${Math.round((topPassive.count / totals.pals) * 100)}% of the server runs "${passiveName(topPassive.skill)}". Creativity: zero. Effectiveness: honestly, fair.`,
							`The "${passiveName(topPassive.skill)}" lobby now represents ${Math.round((topPassive.count / totals.pals) * 100)}% of the population. Elections are a formality at this point.`
						])
					}
				: null,
		() =>
			topPassive && secondPassive
				? {
						kind: 'science',
						text: `The one-two punch of "${passiveName(topPassive.skill)}" and "${passiveName(secondPassive.skill)}" defines this meta. Textbook. Unoriginal. Effective.`
					}
				: null,

		// ── Actives / battle strategy ──
		() => {
			if (!topActive) return null;
			const meme = MEME_ACTIVES[topActive.skill];
			if (meme) return meme(topActive.count, activeName(topActive.skill));
			if (DRAMATIC_ACTIVES.has(topActive.skill)) {
				return {
					kind: 'commentary',
					text: `"${activeName(topActive.skill)}" on ${fmt(topActive.count)} pals — someone here had an edgy phase and, honestly? It worked.`
				};
			}
			return {
				kind: 'commentary',
				text: pick([
					`The signature move of this server is "${activeName(topActive.skill)}" (${fmt(topActive.count)} owners). Fear it. Or don't. It rarely lands.`,
					`Battle strategy breakdown: ${fmt(topActive.count)} pals know "${activeName(topActive.skill)}". The plan is "${activeName(topActive.skill)}". There is no plan B.`
				])
			};
		},
		() =>
			topPassive && topActive
				? {
						kind: 'science',
						text: `The meta is clear: "${passiveName(topPassive.skill)}" plus "${activeName(topActive.skill)}". Devastating. Predictable. Beautiful.`
					}
				: null,

		// ── Talents ──
		() => {
			const { hp, attack, defense } = composition.talent_avg;
			if (hp <= 0 && attack <= 0 && defense <= 0) return null;
			const spread = [hp, attack, defense];
			if (Math.max(...spread) - Math.min(...spread) < 2) {
				return {
					kind: 'conspiracy',
					text: `HP, attack and defense talent averages are nearly identical (${hp.toFixed(1)} / ${attack.toFixed(1)} / ${defense.toFixed(1)}). Nobody plans that. What are you all hiding?`
				};
			}
			if (attack >= defense + 5) {
				return {
					kind: 'roast',
					text: `Average attack talent ${attack.toFixed(1)} vs defense ${defense.toFixed(1)}. This server is a glass cannon factory with a returns department.`
				};
			}
			if (defense >= attack + 5) {
				return {
					kind: 'commentary',
					text: `Average defense talent (${defense.toFixed(1)}) outruns attack (${attack.toFixed(1)}). The strategy is "stand there". Historically effective.`
				};
			}
			if (hp > Math.max(attack, defense)) {
				return {
					kind: 'science',
					text: `HP talents lead the averages (${hp.toFixed(1)}). The plan is simple: outlive the problem.`
				};
			}
			return null;
		},

		// ── Levels ──
		() =>
			totals.pals > 0 && composition.avg_level < 15
				? {
						kind: 'roast',
						text: `The average pal level is ${composition.avg_level.toFixed(1)}. These are babies. You are running a kindergarten with bosses.`
					}
				: null,
		() =>
			totals.pals > 0 && composition.avg_level > 40
				? {
						kind: 'roast',
						text: `Average pal level ${composition.avg_level.toFixed(1)}. Somebody carried this group project, and the rest of you know exactly who.`
					}
				: null,
		() => {
			if (!biggestBracket || biggestBracket.count === 0) return null;
			if (biggestBracket.label === '1-20') {
				return {
					kind: 'roast',
					text: `Most pals cluster in the ${biggestBracket.label} bracket (${fmt(biggestBracket.count)} of them). Growth mindset: pending.`
				};
			}
			if (biggestBracket.label === '61-80') {
				return {
					kind: 'commentary',
					text: `A record ${fmt(biggestBracket.count)} pals live in the ${biggestBracket.label} bracket. Retirement community, but make it jacked.`
				};
			}
			return {
				kind: 'commentary',
				text: `Most pals hang out in the ${biggestBracket.label} range (${fmt(biggestBracket.count)} of them). Nobody wants to grind past 60.`
			};
		},

		// ── Gender demographics ──
		() =>
			composition.gender.unknown > 0
				? {
						kind: 'conspiracy',
						text: pick([
							`${fmt(composition.gender.unknown)} pals listing their gender as "unknown". Witness protection? Divine mystery? Radical privacy? We respect it either way.`,
							`${fmt(composition.gender.unknown)} pals checked "prefer not to say". The census people have simply stopped asking.`
						])
					}
				: null,
		() => {
			const total = composition.gender.male + composition.gender.female;
			if (total <= 0) return null;
			const malePct = (composition.gender.male / total) * 100;
			const femalePct = (composition.gender.female / total) * 100;
			if (malePct >= 75 || femalePct >= 75) {
				const side = malePct >= 75 ? 'male' : 'female';
				return {
					kind: 'nature',
					text: `The population skews ${Math.round(Math.max(malePct, femalePct))}% ${side}. The dating scene is, scientifically speaking, a crisis.`
				};
			}
			return null;
		},

		// ── Health & safety ──
		() =>
			condition.sick_pals > 0
				? {
						kind: 'hr',
						text: pick([
							`${fmt(condition.sick_pals)} sick pals. Someone clearly skipped the vitamin-berry aisle at the Pal merchant.`,
							`Occupational hazard report: ${fmt(condition.sick_pals)} pals down with the sniffles. Paid sick days remain theoretical.`
						])
					}
				: null,
		() =>
			condition.fainted_pals > 0
				? {
						kind: 'roast',
						text: pick([
							`${fmt(condition.fainted_pals)} fainted pals. Did the server survive a raid, or a very aggressive picnic?`,
							`${fmt(condition.fainted_pals)} pals lying facedown in the grass. Very metal. Very nappable.`
						])
					}
				: null,
		() =>
			condition.sick_pals === 0 && condition.fainted_pals === 0 && totals.pals > 0
				? {
						kind: 'conspiracy',
						text: pick([
							'No sick or fainted pals. Everyone is thriving. Suspiciously thriving.',
							'Zero illness, zero faintings. Either flawless management or a cover-up. Both are impressive.'
						])
					}
				: null,

		// ── The bouncer report ──
		() =>
			stats.anomalies.danger_count > 0
				? {
						kind: 'conspiracy',
						text: `${fmt(stats.anomalies.danger_count)} pals flagged ILLEGAL. The bouncer is holding a stack of fake IDs and asking zero questions. Yet.`
					}
				: null,
		() =>
			stats.anomalies.pal_count > 0 && stats.anomalies.danger_count === 0
				? {
						kind: 'conspiracy',
						text: `${fmt(stats.anomalies.pal_count)} pals got side-eyed by the legality scanner. Innocent until proven patched.`
					}
				: null,

		// ── Staffing & real estate ──
		() =>
			totals.human_npcs > totals.players && totals.human_npcs > 0
				? {
						kind: 'hr',
						text: `${fmt(totals.human_npcs)} human NPCs versus ${fmt(totals.players)} actual players. The staff outnumber the management. The union meeting is Thursday; snacks provided.`
					}
				: totals.human_npcs > 0
					? {
							kind: 'hr',
							text: `${fmt(totals.human_npcs)} human NPCs employed. The staffing agency is thriving; HR is not.`
						}
					: null,
		() =>
			totals.guilds > 1
				? {
						kind: 'commentary',
						text: pick([
							`${fmt(totals.guilds)} guilds on the server, and they still can't coordinate a raid schedule. Classic.`,
							`${fmt(totals.guilds)} guilds, one island, zero shared calendars. Diplomacy is conducted via turf war.`
						])
					}
				: null,
		() =>
			totals.guilds === 1 && totals.players > 1
				? {
						kind: 'roast',
						text: 'One guild runs this entire server. Democracy was fun while it lasted.'
					}
				: null,
		() =>
			totals.guilds === 0 && totals.players > 1
				? {
						kind: 'roast',
						text: `${fmt(totals.players)} players, zero guilds. Solitude is a lifestyle and also a red flag.`
					}
				: null,
		() =>
			totals.bases > 0 && totals.guilds > 0
				? {
						kind: 'roast',
						text: `${fmt(totals.bases)} bases across ${fmt(totals.guilds)} guilds — that's ${(totals.bases / totals.guilds).toFixed(1)} per guild. Suburbs, but with more artillery.`
					}
				: totals.bases > 0
					? {
							kind: 'roast',
							text: `${fmt(totals.bases)} bases on record and not one garage for the Jetragon. Priorities.`
						}
					: null,
		() =>
			totals.players > 0 && totals.containers / totals.players >= 5
				? {
						kind: 'roast',
						text: pick([
							`${fmt(totals.containers)} containers for ${fmt(totals.players)} players. Hoarders Anonymous meets at the Palbox — bring your own storage.`,
							`Container math: ${fmt(totals.containers)} boxes. At this point "organized" and "buried" are the same word.`
						])
					}
				: null,
		() =>
			totals.players > 0 && totals.containers === 0
				? {
						kind: 'roast',
						text: 'Zero containers on record. Minimalism, or denial? The floor says denial.'
					}
				: null,
		() =>
			palsPerPlayer >= 20
				? {
						kind: 'roast',
						text: `Each player averages ${fmt(palsPerPlayer)} pals. That's less "team" and more "small nation with a flag".`
					}
				: null,

		// ── The mascot ──
		() => {
			if (!mascot) return null;
			const name = names.species(mascot.key);
			if (mascotShare >= 0.15 && totals.pals >= 20) {
				return {
					kind: 'roast',
					text: pick([
						`${Math.round(mascotShare * 100)}% of every pal on this server is a ${name}. We get it. You like ${name}.`,
						`The ${name} lobby now represents ${Math.round(mascotShare * 100)}% of the population. Elections are a formality.`
					])
				};
			}
			return {
				kind: 'nature',
				text: pick([
					`Here we observe the ${name} in its natural habitat: everywhere. ${fmt(mascot.count)} of them, gently ignoring personal space.`,
					`And here, the majestic ${name} — ${fmt(mascot.count)} strong — demonstrates why biodiversity is more of a suggestion here.`
				])
			};
		},
		() =>
			runnerUp && mascot
				? {
						kind: 'nature',
						text: `The unofficial server mascot is ${names.species(mascot.key)} (${fmt(mascot.count)} sightings). Runner-up: ${names.species(runnerUp.key)} with ${fmt(runnerUp.count)}. Solid try, ${names.species(runnerUp.key)}. The academy will call.`
					}
				: null,

		// ── Leaderboard drama ──
		() => {
			if (!top) return null;
			if (top.level != null && top.pal_count > 0) {
				return {
					kind: 'commentary',
					text: pick([
						`Oi, ${top.nickname || '(someone)'} — level ${fmt(top.level)}, ${fmt(top.pal_count)} pals in tow. Pretty strong, ya know. Don't let it go to your head.`,
						`${top.nickname || '(someone)'} walks in at level ${fmt(top.level)} with ${fmt(top.pal_count)} pals like the final boss of the leaderboard. The rest of the server is taking notes. And damage.`
					])
				};
			}
			return {
				kind: 'commentary',
				text: `Oi, ${top.nickname || '(someone)'} — nice save. Very impressive. The palbox thinks so too.`
			};
		},
		() => {
			if (!top || !second) return null;
			const gap = top.pal_count - second.pal_count;
			if (gap >= 0 && gap <= 2) {
				return {
					kind: 'commentary',
					text: `${top.nickname || 'First place'} leads ${second.nickname || 'second place'} by ${fmt(gap)} pal${gap === 1 ? '' : 's'}. Sleep with one eye open — ${second.nickname || 'they'}'re coming.`
				};
			}
			if (gap >= 3 && second.pal_count > 0 && top.pal_count / second.pal_count >= 3) {
				return {
					kind: 'roast',
					text: `${top.nickname || 'First place'} owns ${fmt(top.pal_count)} pals. Second place owns ${fmt(second.pal_count)}. The wealth gap is shameful and the guild bank agrees.`
				};
			}
			return null;
		},

		// ── Census corner ──
		() =>
			totals.species > 0 && totals.pals > 0 && totals.species / Math.max(totals.pals, 1) > 0.02
				? {
						kind: 'science',
						text: pick([
							`${fmt(totals.species)} species in one world. The Paldeck is basically complete — flex on your friends.`,
							`Biodiversity report: ${fmt(totals.species)} species coexisting. Attenborough could never.`
						])
					}
				: null,
		() =>
			totals.species > 0 && totals.species <= 5 && totals.pals >= 30
				? {
						kind: 'roast',
						text: `${fmt(totals.species)} species across ${fmt(totals.pals)} pals. Biodiversity said "nah, we're good".`
					}
				: null,
		() =>
			totals.players === 1
				? {
						kind: 'roast',
						text: `One player, ${fmt(totals.pals)} pals. Every voice chat here is a monologue with sound effects.`
					}
				: null,
		() =>
			totals.players === 0
				? {
						kind: 'conspiracy',
						text: 'Zero players on record. The pals have the place to themselves and, frankly, it shows.'
					}
				: null,
		() =>
			totals.pals === 0
				? {
						kind: 'roast',
						text: 'Zero pals on this server. Bold strategy. The grass must be immaculate.'
					}
				: null,

		// ── The prophecy desk ──
		() =>
			totals.pals > 0 && mascot
				? {
						kind: 'prophecy',
						text: pick([
							`The stars foretell: someone here is three breeding sessions from a perfect ${names.species(mascot.key)}. Choose your sacrifices wisely.`,
							`I see great things in your future… specifically, more of the ${biggestBracket?.label ?? 'leveled'} bracket. The spirits do not lie. They procrastinate.`
						])
					}
				: null,
		() =>
			composition.talent_avg.defense > 0 && composition.talent_avg.defense < 60
				? {
						kind: 'prophecy',
						text: 'The prophecy is clear: your defense talents will matter exactly when you least expect. The stars say "good luck". The stars are liars.'
					}
				: null,

		// ── Weather desk ──
		() =>
			topActive && topPassive
				? {
						kind: 'weather',
						text: pick([
							`Today's forecast: 90% chance of "${activeName(topActive.skill)}" with a "${passiveName(topPassive.skill)}" advisory in the afternoon. Carry an umbrella. Or a shield.`,
							`Weekend outlook: scattered "${activeName(topActive.skill)}" with heavy "${passiveName(topPassive.skill)}" moving in from the coast. Travel not advised for the squishy.`
						])
					}
				: null,

		// ── The research division ──
		() => ({
			kind: 'science',
			text: pick([
				'This certified scientific report was produced by the Overview Full Mode research division. Yes, it is that serious.',
				'All findings peer-reviewed by the Overview Full Mode research division and one (1) Lamball.',
				'Methodology: vibes, mostly. Margin of error: yes.',
				'Results verified by the Overview Full Mode research division — three pals in a lab coat, one clipboard, zero accountability.'
			])
		})
	];
}

export interface ShenaniganOptions {
	/** How many shenanigans to serve. */
	count?: number;
	/** Texts currently on display — the reroll avoids repeating them. */
	avoid?: string[];
}

/**
 * Rolls a fresh batch of shenanigans. Every eligible generator runs (many
 * randomize their own phrasing), the results shuffle, and `avoid`ed texts are
 * skipped first so consecutive rolls feel like a new narrator took over.
 */
export function generateShenanigans(
	stats: OverviewStats,
	names: SkillNames,
	options: ShenaniganOptions = {}
): Shenanigan[] {
	const count = Math.max(1, options.count ?? 3);
	const avoid = new Set(options.avoid ?? []);

	const pool: Shenanigan[] = [];
	const seen = new Set<string>();
	for (const generate of buildGenerators(stats, names)) {
		const shenanigan = generate();
		if (shenanigan && shenanigan.text.trim().length > 0 && !seen.has(shenanigan.text)) {
			seen.add(shenanigan.text);
			pool.push(shenanigan);
		}
	}

	for (let i = pool.length - 1; i > 0; i -= 1) {
		const j = Math.floor(Math.random() * (i + 1));
		[pool[i], pool[j]] = [pool[j], pool[i]];
	}

	const fresh = pool.filter((s) => !avoid.has(s.text));
	const chosen = (fresh.length >= count ? fresh : pool).slice(0, count);
	return chosen;
}
