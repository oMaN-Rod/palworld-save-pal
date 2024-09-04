<script lang="ts">
	import { Slider } from '@skeletonlabs/skeleton-svelte';
	import type { Pal } from '$types';
	import { Input } from '$components/ui';

	let { pal = $bindable() } = $props<{
		pal: Pal;
	}>();

	let hp: number[] = $state([0]);
	let attack: number[] = $state([0]);
	let defense: number[] = $state([0]);
	let craftspeed: number[] = $state([0]);

	$effect(() => {
		hp = [pal.rank_hp] ?? 0;
		attack = [pal.rank_attack] ?? 0;
		defense = [pal.rank_defense] ?? 0;
		craftspeed = [pal.rank_craftspeed] ?? 0;
	});

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
	<span class="w-3/12">Health</span>
	<Slider
		classes="grow"
		height="h-0.5"
		meterBg="bg-green-500"
		thumbRingColor="ring-green-500"
		min={0}
		max={10}
		markers={[2, 4, 6, 8]}
		step={1}
		bind:value={hp}
		onValueChange={handleUpdateHp}
	/>
	<Input
		type="number"
		labelClass="w-24"
		inputClass="h-8"
		bind:value={hp[0]}
		on:change={handleUpdateHp}
	/>
</div>
<div class="flex flex-row items-center space-x-2">
	<span class="w-3/12">Defense</span>
	<Slider
		height="h-0.5"
		meterBg="bg-primary-500"
		thumbRingColor="ring-primary-500"
		min={0}
		max={10}
		markers={[2, 4, 6, 8]}
		step={1}
		bind:value={defense}
		onValueChange={handleUpdateDefense}
	/>
	<Input
		type="number"
		labelClass="w-24"
		inputClass="h-8"
		bind:value={defense[0]}
		on:change={handleUpdateDefense}
	/>
</div>
<div class="flex flex-row items-center space-x-2">
	<span class="w-3/12">Attack</span>
	<Slider
		height="h-0.5"
		meterBg="bg-red-500"
		thumbRingColor="ring-red-500"
		min={0}
		max={10}
		markers={[2, 4, 6, 8]}
		step={1}
		bind:value={attack}
		onValueChange={handleUpdateAttack}
	/>
	<Input
		type="number"
		labelClass="w-24"
		inputClass="h-8"
		bind:value={attack[0]}
		on:change={handleUpdateAttack}
	/>
</div>
<div class="flex flex-row items-center space-x-2">
	<span class="w-3/12">Workspeed</span>
	<Slider
		height="h-0.5"
		meterBg="bg-secondary-500"
		thumbRingColor="ring-secondary-500"
		min={0}
		max={10}
		markers={[2, 4, 6, 8]}
		step={1}
		bind:value={craftspeed}
		onValueChange={handleUpdateCraftSpeed}
	/>
	<Input
		type="number"
		labelClass="w-24"
		inputClass="h-8"
		bind:value={craftspeed[0]}
		on:change={handleUpdateCraftSpeed}
	/>
</div>
