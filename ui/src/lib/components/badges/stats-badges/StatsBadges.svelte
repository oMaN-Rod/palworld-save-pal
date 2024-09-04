<script lang="ts">
	import { assetLoader } from '$lib/utils/asset-loader';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { EntryState, type Pal, type Player } from '$types';
	import { getStats } from '$lib/data';
	import { SectionHeader } from '$components/ui';
	import { HealthBadge } from '$components';

	let {
		pal = $bindable(),
		player = $bindable()
	}: { pal: Pal | undefined; player: Player | undefined } = $props();

	type Stat = {
		name: string;
		value: number;
	};

	let stats: Stat[] = $state([]);
	let foodIcon: string = $state('');
	let hpIcon: string = $state('');

	async function loadStaticIcons() {
		const foodPath = `${ASSET_DATA_PATH}/img/icons/Food.png`;
		const food = await assetLoader.loadImage(foodPath);
		foodIcon = food;

		const hpPath = `${ASSET_DATA_PATH}/img/icons/Heart.png`;
		const hp = await assetLoader.loadImage(hpPath);
		hpIcon = hp;
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

	function handleHeal() {
		if (!pal) return;
		pal.hp = pal.max_hp;
		pal.state = EntryState.MODIFIED;
	}

	function handleEat() {
		if (!pal) return;
		pal.stomach = pal.max_stomach;
		pal.state = EntryState.MODIFIED;
	}

	async function handleGetStats() {
		if (pal && player) {
			stats = await getStats(pal, player);
		}
	}

	$effect(() => {
		handleGetStats();
		loadStaticIcons();
	});

	$effect(() => {
		if (
			pal &&
			player &&
			(pal?.talent_hp || pal?.talent_melee || pal?.talent_defense || pal?.passive_skills)
		) {
			console.log('Talent hp:', pal.talent_hp);
			console.log('Talent melee:', pal.talent_melee);
			console.log('Talent defense:', pal.talent_defense);
			console.log('Passive skills:', pal.passive_skills);
			handleGetStats();
		}
	});
</script>

<HealthBadge {pal} {player} />
<SectionHeader text="Stats" />
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
