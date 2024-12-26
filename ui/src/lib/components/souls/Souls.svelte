<script lang="ts">
	import { Slider } from '@skeletonlabs/skeleton-svelte';
	import type { Pal } from '$types';
	import { Input } from '$components/ui';

	let { pal = $bindable() } = $props<{
		pal: Pal;
	}>();

	let hp = $derived([pal.rank_hp ?? 0]);
	let attack = $derived([pal.rank_attack ?? 0]);
	let defense = $derived([pal.rank_defense ?? 0]);
	let craftspeed = $derived([pal.rank_craftspeed ?? 0]);

	function handleUpdateHp(details: any): void {
		pal.rank_hp = details.value[0];
	}

	function handleUpdateAttack(details: any): void {
		pal.rank_attack = details.value[0];
	}

	function handleUpdateCraftSpeed(details: any): void {
		pal.rank_craftspeed = details.value[0];
	}

	function handleUpdateDefense(details: any): void {
		pal.rank_defense = details.value[0];
	}
</script>

<div class="flex flex-row items-center space-x-2">
	<span class="ml-2 w-3/12">Health</span>
	<Slider
		classes="grow"
		height="h-0.5"
		meterBg="bg-green-500"
		thumbRingColor="ring-green-500"
		min={0}
		max={20}
		markers={[5, 10, 15]}
		step={1}
		value={hp}
		onValueChange={handleUpdateHp}
	/>
	<Input
		type="number"
		labelClass="w-24"
		inputClass="h-8"
		value={hp[0]}
		onchange={handleUpdateHp}
		min={0}
		max={20}
	/>
</div>
<div class="flex flex-row items-center space-x-2">
	<span class="ml-2 w-3/12">Defense</span>
	<Slider
		height="h-0.5"
		meterBg="bg-primary-500"
		thumbRingColor="ring-primary-500"
		min={0}
		max={20}
		markers={[5, 10, 15]}
		step={1}
		value={defense}
		onValueChange={handleUpdateDefense}
	/>
	<Input
		type="number"
		labelClass="w-24"
		inputClass="h-8"
		value={defense[0]}
		onchange={handleUpdateDefense}
		min={0}
		max={20}
	/>
</div>
<div class="flex flex-row items-center space-x-2">
	<span class="ml-2 w-3/12">Attack</span>
	<Slider
		height="h-0.5"
		meterBg="bg-red-500"
		thumbRingColor="ring-red-500"
		min={0}
		max={20}
		markers={[5, 10, 15]}
		step={1}
		value={attack}
		onValueChange={handleUpdateAttack}
	/>
	<Input
		type="number"
		labelClass="w-24"
		inputClass="h-8"
		value={attack[0]}
		onchange={handleUpdateAttack}
		min={0}
		max={20}
	/>
</div>
<div class="flex flex-row items-center space-x-2">
	<span class="ml-2 w-3/12">Workspeed</span>
	<Slider
		height="h-0.5"
		meterBg="bg-secondary-500"
		thumbRingColor="ring-secondary-500"
		min={0}
		max={20}
		markers={[5, 10, 15]}
		step={1}
		value={craftspeed}
		onValueChange={handleUpdateCraftSpeed}
	/>
	<Input
		type="number"
		labelClass="w-24"
		inputClass="h-8"
		value={craftspeed[0]}
		onchange={handleUpdateCraftSpeed}
		min={0}
		max={20}
	/>
</div>
