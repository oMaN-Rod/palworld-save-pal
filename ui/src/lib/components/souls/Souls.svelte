<script lang="ts">
	import { Slider } from '@skeletonlabs/skeleton-svelte';
	import { EntryState, type Pal } from '$types';
	import { Input } from '$components/ui';
	import { getAppState } from '$states';

	let { pal = $bindable() } = $props<{
		pal: Pal;
	}>();

	let appstate = getAppState();
	let max: number = $state(0);
	let markers: number[] = $state([]);

	const hp = $derived([pal.rank_hp ?? 0]);
	const attack = $derived([pal.rank_attack ?? 0]);
	const defense = $derived([pal.rank_defense ?? 0]);
	const craftSpeed = $derived([pal.rank_craftspeed ?? 0]);

	function handleUpdateHp(details: any): void {
		pal.rank_hp = details.value[0];
		pal.state = EntryState.MODIFIED;
	}

	function handleUpdateAttack(details: any): void {
		pal.rank_attack = details.value[0];
		pal.state = EntryState.MODIFIED;
	}

	function handleUpdateCraftSpeed(details: any): void {
		pal.rank_craftspeed = details.value[0];
		pal.state = EntryState.MODIFIED;
	}

	function handleUpdateDefense(details: any): void {
		pal.rank_defense = details.value[0];
		pal.state = EntryState.MODIFIED;
	}

	async function updateSettings() {
		if (appstate.settings.cheat_mode) {
			max = 255;
			markers = [50, 100, 150, 200];
		} else {
			max = 20;
			markers = [5, 10, 15];
		}
	}

	$effect(() => {
		updateSettings();
	});
</script>

<div class="grid grid-cols-[80px_1fr_auto] items-center gap-2">
	<span>Health</span>
	<Slider
		classes="grow"
		height="h-2"
		meterBg="bg-green-500"
		thumbRingColor="ring-green-500"
		min={0}
		max={max}
		markers={markers}
		step={1}
		value={hp}
		onValueChange={handleUpdateHp}
	/>
	<Input
		type="number"
		inputClass="h-8 p-1"
		value={hp[0]}
		onchange={handleUpdateHp}
		min={0}
		max={max}
	/>

	<span>Attack</span>
	<Slider
		height="h-2"
		meterBg="bg-red-500"
		thumbRingColor="ring-red-500"
		min={0}
		max={max}
		markers={markers}
		step={1}
		value={attack}
		onValueChange={handleUpdateAttack}
	/>
	<Input
		type="number"
		inputClass="h-8 p-1"
		value={attack[0]}
		onchange={handleUpdateAttack}
		min={0}
		max={max}
	/>

	<span>Defense</span>
	<Slider
		height="h-2"
		meterBg="bg-primary-500"
		thumbRingColor="ring-primary-500"
		min={0}
		max={max}
		markers={markers}
		step={1}
		value={defense}
		onValueChange={handleUpdateDefense}
	/>
	<Input
		type="number"
		inputClass="h-8 p-1"
		value={defense[0]}
		onchange={handleUpdateDefense}
		min={0}
		max={max}
	/>

	<span>Workspeed</span>
	<Slider
		height="h-2"
		meterBg="bg-secondary-500"
		thumbRingColor="ring-secondary-500"
		min={0}
		max={max}
		markers={markers}
		step={1}
		value={craftSpeed}
		onValueChange={handleUpdateCraftSpeed}
	/>
	<Input
		type="number"
		inputClass="h-8 p-1"
		value={craftSpeed[0]}
		onchange={handleUpdateCraftSpeed}
		min={0}
		max={max}
	/>
</div>
