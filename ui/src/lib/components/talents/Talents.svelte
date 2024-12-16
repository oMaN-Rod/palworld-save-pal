<script lang="ts">
	import { Slider } from '@skeletonlabs/skeleton-svelte';
	import type { Pal } from '$types';
	import { Input } from '$components/ui';

	let { pal = $bindable() } = $props<{
		pal: Pal;
	}>();

	let hp = $derived([pal.talent_hp ?? 0]);
	let attack = $derived([pal.talent_shot ?? 0]);
	let defense = $derived([pal.talent_defense ?? 0]);

	function handleUpdateHp(details: any): void {
		pal.talent_hp = details.value[0];
	}

	function handleUpdateAttack(details: any): void {
		pal.talent_shot = details.value[0];
	}

	function handleUpdateDefense(details: any): void {
		pal.talent_defense = details.value[0];
	}
</script>

<div class="flex flex-row items-center space-x-2">
	<span class="ml-2 w-3/12">HP</span>
	<Slider
		classes="grow"
		height="h-0.5"
		meterBg="bg-green-500"
		thumbRingColor="ring-green-500"
		min={0}
		max={100}
		markers={[25, 50, 75]}
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
		max={100}
	/>
</div>
<div class="flex flex-row items-center space-x-2">
	<span class="ml-2 w-3/12">Attack</span>
	<Slider
		height="h-0.5"
		meterBg="bg-red-500"
		thumbRingColor="ring-red-500"
		min={0}
		max={100}
		markers={[25, 50, 75]}
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
		max={100}
	/>
</div>
<div class="flex h-8 flex-row items-center space-x-2">
	<span class="ml-2 w-3/12">Defense</span>
	<Slider
		height="h-0.5"
		meterBg="bg-primary-500"
		thumbRingColor="ring-primary-500"
		min={0}
		max={100}
		markers={[25, 50, 75]}
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
		max={100}
	/>
</div>
