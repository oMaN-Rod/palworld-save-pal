<script lang="ts">
	import { EntryState, type Player } from '$types';
	import { Tooltip, Progress } from '$components/ui';
	import { staticIcons } from '$types/icons';

	let {
		player = $bindable(),
		maxHp = $bindable()
	}: {
		player: Player;
		maxHp: number;
	} = $props();

	function handleHeal() {
		player.hp = maxHp * 1000;
		player.stomach = 100;
		player.state = EntryState.MODIFIED;
	}

	async function handleEat() {
		player.stomach = 100;
		player.state = EntryState.MODIFIED;
	}
</script>

<div class="flex flex-col space-y-1">
	<div class="flex flex-row items-center">
		<Tooltip>
			<button onclick={handleHeal} aria-label="Health">
				<img src={staticIcons.hpIcon} alt="Health" class="mr-2 h-6 w-6" />
			</button>

			{#snippet popup()}
				<div class="flex flex-col">
					<span class="font-bold">Restore HP</span>
					<span>{Math.round(player.hp / 1000)} / {maxHp}</span>
				</div>
			{/snippet}
		</Tooltip>
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
		<Tooltip>
			<button class="mr-2" onclick={handleEat} aria-label="Food">
				<img src={staticIcons.foodIcon} alt="Food" class="h-6 w-6" />
			</button>
			{#snippet popup()}
				<div class="flex flex-col">
					<span class="font-bold">Stomach</span>
					<span>{Math.round(player.stomach)} / 100</span>
				</div>
			{/snippet}
		</Tooltip>

		<Progress bind:value={player.stomach} max={100} height="h-6" color="orange" width="w-[280px]" />
	</div>
</div>
