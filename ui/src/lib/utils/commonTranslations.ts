import * as m from '$i18n/messages';

/**
 * Common translations used throughout the application.
 * Centralizes frequently used translation strings to reduce code repetition
 * and ensure consistency across components.
 *
 * @example
 * ```ts
 * import { c } from '$lib/utils/commonTranslations';
 * console.log(c.pal); // Outputs singular "Pal" translation
 * console.log(c.pals); // Outputs plural "Pals" translation
 * ```
 */

export const c = {
	activeSkill: m.active_skill({ count: 1 }),
	activeSkills: m.active_skill({ count: 2 }),
	alphaPal: m.alpha_pal({ pals: m.pal({ count: 1 }) }),
	alphaPals: m.alpha_pal({ pals: m.pal({ count: 2 }) }),
	base: m.base({ count: 1 }),
	bases: m.base({ count: 2 }),
	dimensionalPalStorage: m.dimensional_pal_storage({ pal: m.pal({ count: 1 }) }),
	globalPalStorage: m.global_pal_storage({ pal: m.pal({ count: 1 }) }),
	guild: m.guild({ count: 1 }),
	guilds: m.guild({ count: 2 }),
	human: m.human({ count: 1 }),
	humans: m.human({ count: 2 }),
	item: m.item({ count: 1 }),
	items: m.item({ count: 2 }),
	luckyPal: m.lucky_pals({ pals: m.pal({ count: 1 }) }),
	luckyPals: m.lucky_pals({ pals: m.pal({ count: 2 }) }),
	oilRigPal: m.oil_rig_pals({ pals: m.pal({ count: 1 }) }),
	oilRigPals: m.oil_rig_pals({ pals: m.pal({ count: 2 }) }),
	pal: m.pal({ count: 1 }),
	pals: m.pal({ count: 2 }),
	passiveSkill: m.passive_skill({ count: 1 }),
	passiveSkills: m.passive_skill({ count: 2 }),
	player: m.player({ count: 1 }),
	players: m.player({ count: 2 }),
	predatorPal: m.predator_pals({ pals: m.pal({ count: 1 }) }),
	predatorPals: m.predator_pals({ pals: m.pal({ count: 2 }) }),
	preset: m.preset({ count: 1 }),
	presets: m.preset({ count: 2 }),
	save: m.save({ count: 1 }),
	saves: m.save({ count: 2 }),
	tag: m.tag({ count: 1 }),
	tags: m.tag({ count: 2 }),
	summonedPal: m.summoned_pals({ pals: m.pal({ count: 1 }) }),
	summonedPals: m.summoned_pals({ pals: m.pal({ count: 2 }) }),
	universalPalStorage: m.universal_pal_storage({ pal: m.pal({ count: 1 }) }),
	weapon: m.weapon({ count: 1 }),
	weapons: m.weapon({ count: 2 }),
	collection: m.collection({ count: 1 }),
	collections: m.collection({ count: 2 }),
	filter: m.filter({ count: 1 }),
	filters: m.filter({ count: 2 }),
	container: m.storage_container(),
	storage: m.storage()
};

/**
 * Pre-formatted parameter objects for message functions that require
 * common translation values as arguments.
 * These objects provide ready-to-use parameter sets to avoid repeatedly
 * constructing the same parameter structures throughout the application.
 *
 * @example
 * ```ts
 * import { p } from '$lib/utils/commonTranslations';
 * m.some_message(p.pal); // Passes { pal: m.pal({ count: 1 }) } as parameter
 * ```
 */

export const p = {
	pal: {
		pal: c.pal
	},
	pals: {
		pals: c.pals
	},
	human: {
		human: c.human
	},
	humans: {
		humans: c.humans
	}
};
