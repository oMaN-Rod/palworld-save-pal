import * as m from '$i18n/messages';

export const c = {
	get activeSkill() {
		return m.active_skill({ count: 1 });
	},
	get activeSkills() {
		return m.active_skill({ count: 2 });
	},
	get alphaPal() {
		return m.alpha_pal({ pals: m.pal({ count: 1 }) });
	},
	get alphaPals() {
		return m.alpha_pal({ pals: m.pal({ count: 2 }) });
	},
	get base() {
		return m.base({ count: 1 });
	},
	get bases() {
		return m.base({ count: 2 });
	},
	get dimensionalPalStorage() {
		return m.dimensional_pal_storage({ pal: m.pal({ count: 1 }) });
	},
	get globalPalStorage() {
		return m.global_pal_storage({ pal: m.pal({ count: 1 }) });
	},
	get guild() {
		return m.guild({ count: 1 });
	},
	get guilds() {
		return m.guild({ count: 2 });
	},
	get human() {
		return m.human({ count: 1 });
	},
	get humans() {
		return m.human({ count: 2 });
	},
	get item() {
		return m.item({ count: 1 });
	},
	get items() {
		return m.item({ count: 2 });
	},
	get luckyPal() {
		return m.lucky_pals({ pals: m.pal({ count: 1 }) });
	},
	get luckyPals() {
		return m.lucky_pals({ pals: m.pal({ count: 2 }) });
	},
	get oilRigPal() {
		return m.oil_rig_pals({ pals: m.pal({ count: 1 }) });
	},
	get oilRigPals() {
		return m.oil_rig_pals({ pals: m.pal({ count: 2 }) });
	},
	get pal() {
		return m.pal({ count: 1 });
	},
	get pals() {
		return m.pal({ count: 2 });
	},
	get passiveSkill() {
		return m.passive_skill({ count: 1 });
	},
	get passiveSkills() {
		return m.passive_skill({ count: 2 });
	},
	get player() {
		return m.player({ count: 1 });
	},
	get players() {
		return m.player({ count: 2 });
	},
	get predatorPal() {
		return m.predator_pals({ pals: m.pal({ count: 1 }) });
	},
	get predatorPals() {
		return m.predator_pals({ pals: m.pal({ count: 2 }) });
	},
	get preset() {
		return m.preset({ count: 1 });
	},
	get presets() {
		return m.preset({ count: 2 });
	},
	get save() {
		return m.save({ count: 1 });
	},
	get saves() {
		return m.save({ count: 2 });
	},
	get tag() {
		return m.tag({ count: 1 });
	},
	get tags() {
		return m.tag({ count: 2 });
	},
	get summonedPal() {
		return m.summoned_pals({ pals: m.pal({ count: 1 }) });
	},
	get summonedPals() {
		return m.summoned_pals({ pals: m.pal({ count: 2 }) });
	},
	get universalPalStorage() {
		return m.universal_pal_storage({ pal: m.pal({ count: 1 }) });
	},
	get weapon() {
		return m.weapon({ count: 1 });
	},
	get weapons() {
		return m.weapon({ count: 2 });
	},
	get collection() {
		return m.collection({ count: 1 });
	},
	get collections() {
		return m.collection({ count: 2 });
	},
	get filter() {
		return m.filter({ count: 1 });
	},
	get filters() {
		return m.filter({ count: 2 });
	},
	get container() {
		return m.storage_container();
	},
	get storage() {
		return m.storage();
	}
};

export const p = {
	get pal() {
		return { pal: c.pal };
	},
	get pals() {
		return { pals: c.pals };
	},
	get human() {
		return { human: c.human };
	},
	get humans() {
		return { humans: c.humans };
	}
};
