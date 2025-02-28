<script lang="ts">
	import { type Pal, type Player } from '$types';
	import { getStats, type PalStats } from '$lib/data';
	import { staticIcons } from '$lib/constants';

	let {
		pal = $bindable(),
		player = $bindable()
	}: { pal: Pal | undefined; player: Player | undefined } = $props();

	let stats: PalStats | undefined = $state();

	$effect(() => {
		if (pal && player) {
			stats = getStats(pal, player);
		}
	});
</script>

<div class="flex flex-col space-y-2">
	<div
		class="border-l-primary border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none"
	>
		<div class="flex w-full items-center">
			<img src={staticIcons.attackIcon} alt="HP" class="mx-2 h-6 w-6" />
			<div class="ml-2 h-6 w-6"></div>
			<span class="grow p-2 text-lg">Attack</span>
			<span class="p-2 text-lg font-bold">{Math.round(stats?.attack || 0)}</span>
		</div>
	</div>
	<div
		class="border-l-primary border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none"
	>
		<div class="flex w-full items-center">
			<img src={staticIcons.defenseIcon} alt="HP" class="mx-2 h-6 w-6" />

			<div class="ml-2 h-6 w-6"></div>
			<span class="grow p-2 pl-2 text-lg">Defense</span>
			<span class="p-2 text-lg font-bold">{Math.round(stats?.defense || 0)}</span>
		</div>
	</div>
	<div
		class="border-l-primary border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none"
	>
		<div class="flex w-full items-center">
			<img src={staticIcons.workSpeedIcon} alt="HP" class="mx-2 h-6 w-6" />

			<div class="ml-2 h-6 w-6"></div>
			<span class="grow p-2 pl-2 text-lg">Work Speed</span>
			<span class="p-2 text-lg font-bold">{Math.round(stats?.workSpeed || 0)}</span>
		</div>
	</div>
</div>
