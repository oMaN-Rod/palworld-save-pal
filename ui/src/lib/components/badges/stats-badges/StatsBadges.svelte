<script lang="ts">
	import { assetLoader } from '$lib/utils/asset-loader';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import type { Pal, Player } from '$types';
	import { palsData, passiveSkillsData } from '$lib/data';

	let {
		pal = $bindable(),
		player = $bindable()
	}: { pal: Pal | undefined; player: Player | undefined } = $props();

	type Stat = {
		name: string;
		value: number;
	};

	async function getStats(): Promise<Stat[]> {
		if (!pal) {
			console.log('No pal provided');
			return [];
		}
		if (!player) {
			console.log('No player provided');
			return [];
		}
		const level = player.level < pal.level ? player.level : pal.level;

		const palData = await palsData.getPalInfo(pal.character_id);
		if (!palData) {
			console.log('No pal data found');
			return [];
		}

		let attackBonus = 0;
		let defenseBonus = 0;
		let workSpeedBonus = 0;
		for (let i = 0; i < pal.passive_skills.length; i++) {
			const skill = pal.passive_skills[i];
			const skillData = await passiveSkillsData.searchPassiveSkills(skill);
			if (!skillData) {
				continue;
			}
			attackBonus += skillData.details.Bonuses.Attack / 100;
			defenseBonus += skillData.details.Bonuses.Defense / 100;
			workSpeedBonus += skillData.details.Bonuses.WorkSpeed / 100;
		}
		const condenserBonus = (pal.rank - 1) * 0.05;

		const hp_iv = (pal.talent_hp * 0.3) / 100;
		const hp_rank = pal.rank_hp * 0.03;
		const hp_scale = palData.scaling.HP;
		let hp = Math.floor(500 + 5 * level + hp_scale * 0.5 * level * (1 + hp_iv));
		hp = Math.floor(hp * (1 + condenserBonus) * (1 + hp_rank));

		const attack_iv = (pal.talent_melee * 0.3) / 100;
		const attack_rank = pal.rank_attack * 0.03;
		const attack_scale = palData.scaling.Attack;

		let attack = Math.floor(attack_scale * 0.075 * level * (1 + attack_iv));
		attack = Math.floor(attack * (1 + condenserBonus) * (1 + attack_rank) * (1 + attackBonus));

		const defense_iv = (pal.talent_defense * 0.3) / 100;
		const defense_rank = pal.rank_defense * 0.03;
		const defense_scale = palData.scaling.Defense;

		let defense = Math.floor(50 + defense_scale * 0.075 * level * (1 + defense_iv));
		defense = Math.floor(defense * (1 + condenserBonus) * (1 + defense_rank) * (1 + defenseBonus));

		return [
			{ name: 'attack', value: attack },
			{ name: 'defense', value: defense },
			{ name: 'work_speed', value: pal.work_speed }
		];
	}

	async function loadSvgContent(stat: string): Promise<string> {
		const svgPath = `${ASSET_DATA_PATH}/img/stats/${stat}.svg`;
		try {
			return await assetLoader.load(svgPath, 'svg');
		} catch (error) {
			console.error(`Failed to load SVG for ${stat}:`, error);
			return '';
		}
	}

	function formatStatText(stat: string): string {
		return stat
			.split('_')
			.map((word) => word.charAt(0).toUpperCase() + word.slice(1))
			.join(' ');
	}
</script>

{#await getStats() then stats}
	{#each stats as stat}
		<div
			class="border-l-primary border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none"
		>
			<div class="flex w-full items-center">
				{#await loadSvgContent(stat.name)}
					<div class="ml-2 h-6 w-6"></div>
				{:then svgContent}
					<div class="mx-2 h-6 w-6">
						{@html svgContent}
					</div>
				{:catch error}
					<div class="ml-2 h-6 w-6"></div>
				{/await}
				<span class="flex-grow p-2 text-lg">{formatStatText(stat.name)}</span>
				<span class="p-2 text-lg font-bold">{stat.value}</span>
			</div>
		</div>
	{/each}
{/await}
