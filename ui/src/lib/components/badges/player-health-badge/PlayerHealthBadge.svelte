<script lang="ts">
	import { assetLoader } from '$lib/utils/asset-loader';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { EntryState, type Pal, type Player } from '$types';
	import { Tooltip, Progress } from '$components/ui';

	let {
		player = $bindable(),
		maxHp = $bindable()
	}: {
		player: Player;
		maxHp: number;
	} = $props();

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
		player.hp = maxHp * 1000;
		player.stomach = 100;
		player.state = EntryState.MODIFIED;
	}

	async function handleEat() {
		player.stomach = 100;
		player.state = EntryState.MODIFIED;
	}

	$effect(() => {
		loadStaticIcons();
	});
</script>

<div class="flex flex-col space-y-1">
	<div class="flex flex-row items-center">
		{#if hpIcon}
			<Tooltip>
				<button onclick={handleHeal} aria-label="Health">
					<enhanced:img src={hpIcon} alt="Health" class="mr-2 h-6 w-6"></enhanced:img>
				</button>

				{#snippet popup()}
					<div class="flex flex-col">
						<span class="font-bold">Restore HP</span>
						<span>{Math.round(player.hp / 1000)} / {maxHp}</span>
					</div>
				{/snippet}
			</Tooltip>
		{/if}
		<Progress
			bind:value={player.hp}
			max={maxHp * 1000}
			height="h-6"
			color="green"
			width="w-[280px]"
			dividend={1000}
		/>
	</div>
	<div class="flex w-full flex-row items-center">
		{#if foodIcon}
			<Tooltip>
				<button class="mr-2" onclick={handleEat} aria-label="Food">
					<enhanced:img src={foodIcon} alt="Food" class="h-6 w-6"></enhanced:img>
				</button>
				{#snippet popup()}
					<div class="flex flex-col">
						<span class="font-bold">Stomach</span>
						<span>{Math.round(player.stomach)} / 100</span>
					</div>
				{/snippet}
			</Tooltip>
		{/if}

		<Progress bind:value={player.stomach} max={100} height="h-6" color="orange" width="w-[280px]" />
	</div>
</div>
