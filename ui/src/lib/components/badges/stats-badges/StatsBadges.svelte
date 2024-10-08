<script lang="ts">
	import { assetLoader } from '$lib/utils/asset-loader';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { EntryState, type Pal, type Player } from '$types';
	import { getStats, type PalStats } from '$lib/data';
	import { SectionHeader } from '$components/ui';
	import { HealthBadge } from '$components';
	import { getModalState } from '$states';
	import { NumberInputModal } from '$components/modals';

	let {
		pal = $bindable(),
		player = $bindable()
	}: { pal: Pal | undefined; player: Player | undefined } = $props();

	const modal = getModalState();

	let stats: PalStats | undefined = $state();
	let attackIcon: string = $state('');
	let defenseIcon: string = $state('');
	let workSpeedIcon: string = $state('');
	let foodIcon: string = $state('');
	let hpIcon: string = $state('');

	async function loadStaticIcons() {
		const foodPath = `${ASSET_DATA_PATH}/img/icons/Food.png`;
		const food = await assetLoader.loadImage(foodPath);
		foodIcon = food;

		const hpPath = `${ASSET_DATA_PATH}/img/icons/Heart.png`;
		const hp = await assetLoader.loadImage(hpPath);
		hpIcon = hp;

		const attackPath = `${ASSET_DATA_PATH}/img/stats/attack.svg`;
		const attack = await assetLoader.load(attackPath, 'svg');
		attackIcon = attack as string;

		const defensePath = `${ASSET_DATA_PATH}/img/stats/defense.svg`;
		const defense = await assetLoader.load(defensePath, 'svg');
		defenseIcon = defense as string;

		const workPath = `${ASSET_DATA_PATH}/img/stats/work_speed.svg`;
		const work = await assetLoader.load(workPath, 'svg');
		workSpeedIcon = work as string;
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
			(pal?.talent_hp || pal?.talent_shot || pal?.talent_defense || pal?.passive_skills)
		) {
			handleGetStats();
		}
	});
</script>

<HealthBadge {pal} {player} />
<SectionHeader text="Stats" />
<div
	class="border-l-primary border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none"
>
	<div class="flex w-full items-center">
		<div class="mx-2 h-6 w-6">
			{@html attackIcon}
		</div>

		<div class="ml-2 h-6 w-6"></div>
		<span class="flex-grow p-2 text-lg">Attack</span>
		<span class="p-2 text-lg font-bold">{stats?.attack}</span>
	</div>
</div>
<div
	class="border-l-primary border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none"
>
	<div class="flex w-full items-center">
		<div class="mx-2 h-6 w-6">
			{@html defenseIcon}
		</div>

		<div class="ml-2 h-6 w-6"></div>
		<span class="flex-grow p-2 pl-2 text-lg">Defense</span>
		<span class="p-2 text-lg font-bold">{stats?.defense}</span>
	</div>
</div>
