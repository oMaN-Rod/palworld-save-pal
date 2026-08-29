<script lang="ts">
	import { type Pal, type Player } from '$types';
	import { getStats, type PalStats } from '$lib/utils';
	import { staticIcons } from '$types/icons';
	import * as m from '$i18n/messages';

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
			<img src={staticIcons.attackIcon} alt={m.attack()} class="mx-2 h-6 w-6" />
			<div class="ml-2 h-6 w-6"></div>
			<span class="grow p-2 text-lg">{m.attack()}</span>
			<span class="p-2 text-lg font-bold">{Math.round(stats?.attack || 0)}</span>
		</div>
	</div>
	<div
		class="border-l-primary border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none"
	>
		<div class="flex w-full items-center">
			<img src={staticIcons.defenseIcon} alt={m.defense()} class="mx-2 h-6 w-6" />

			<div class="ml-2 h-6 w-6"></div>
			<span class="grow p-2 pl-2 text-lg">{m.defense()}</span>
			<span class="p-2 text-lg font-bold">{Math.round(stats?.defense || 0)}</span>
		</div>
	</div>
	<div
		class="border-l-primary border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none"
	>
		<div class="flex w-full items-center">
			<img src={staticIcons.workSpeedIcon} alt={m.work_speed()} class="mx-2 h-6 w-6" />

			<div class="ml-2 h-6 w-6"></div>
			<span class="grow p-2 pl-2 text-lg">{m.work_speed()}</span>
			<span class="p-2 text-lg font-bold">{Math.round(stats?.workSpeed || 0)}</span>
		</div>
	</div>
</div>
