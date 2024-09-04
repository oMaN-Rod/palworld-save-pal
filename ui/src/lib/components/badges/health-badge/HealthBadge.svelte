<script lang="ts">
	import { assetLoader } from '$lib/utils/asset-loader';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { EntryState, type Pal, type Player } from '$types';
	import { getStats } from '$lib/data';
	import { Tooltip, Progress } from '$components/ui';

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

{#if pal}
	<div class="flex flex-row items-center">
		{#if hpIcon}
			<Tooltip>
				<button onclick={handleHeal}>
					<enhanced:img src={hpIcon} alt="Food" class="mr-2 h-6 w-6"></enhanced:img>
				</button>

				{#snippet popup()}
					<span>HP</span>
					{Math.round(pal.hp / 1000)}/{pal.max_hp / 1000}
				{/snippet}
			</Tooltip>
		{/if}
		<Progress
			bind:value={pal.hp}
			bind:max={pal.max_hp}
			height="h-6"
			color="green"
			width="w-[400px]"
			dividend={1000}
		/>
	</div>
	<div class="flex flex-row items-center">
		{#if foodIcon}
			<Tooltip>
				<button class="mr-2" onclick={handleEat}>
					<enhanced:img src={foodIcon} alt="Food" class="h-6 w-6"></enhanced:img>
				</button>
				{#snippet popup()}
					<span>Feed</span>
					{Math.round(pal.stomach)}/{pal.max_stomach}
				{/snippet}
			</Tooltip>
		{/if}

		<Progress
			bind:value={pal.stomach}
			bind:max={pal.max_stomach}
			height="h-6"
			width="w-[400px]"
			color="orange"
		/>
	</div>
	<div
		class="bg-surface-800 text-one-surface hover:ring-secondary-500 relative mx-2 flex h-1/2 rounded px-6 py-1 font-semibold hover:ring"
	>
		<span class="relative z-10 grow">SAN</span>
		<span class="relative z-10">{pal.sanity.toFixed(0)} / 100</span>
		<span class="border-surface-700 absolute inset-0 rounded border"></span>
		<span class="bg-surface-600 absolute left-0 top-0 h-0.5 w-0.5"></span>
		<span class="bg-surface-600 absolute right-0 top-0 h-0.5 w-0.5"></span>
		<span class="bg-surface-600 absolute bottom-0 left-0 h-0.5 w-0.5"></span>
		<span class="bg-surface-600 absolute bottom-0 right-0 h-0.5 w-0.5"></span>
	</div>
{/if}
