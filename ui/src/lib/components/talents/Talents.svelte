<script lang="ts">
	import { Slider } from '@skeletonlabs/skeleton-svelte';
	import { EntryState, type Pal } from '$types';
	import { Input } from '$components/ui';

	let { pal = $bindable() } = $props<{
		pal: Pal;
	}>();

	const hp = $derived([pal.talent_hp ?? 0]);
	const attack = $derived([pal.talent_shot ?? 0]);
	const defense = $derived([pal.talent_defense ?? 0]);

	function handleUpdateHp(details: any): void {
		pal.talent_hp = details.value[0];
		pal.state = EntryState.MODIFIED;
	}

	function handleUpdateAttack(details: any): void {
		pal.talent_shot = details.value[0];
		pal.state = EntryState.MODIFIED;
	}

	function handleUpdateDefense(details: any): void {
		pal.talent_defense = details.value[0];
		pal.state = EntryState.MODIFIED;
	}
</script>

<div class="grid grid-cols-[80px_1fr_auto] items-center gap-2">
	<span>HP</span>
	<Slider
		classes="grow"
		height="h-2"
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
		inputClass="h-8 p-1"
		value={hp[0]}
		onchange={handleUpdateHp}
		min={0}
		max={100}
	/>

	<span>Attack</span>
	<Slider
		height="h-2"
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
		inputClass="h-8 p-1"
		value={attack[0]}
		onchange={handleUpdateAttack}
		min={0}
		max={100}
	/>

	<span>Defense</span>
	<Slider
		height="h-2"
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
		inputClass="h-8 p-1"
		value={defense[0]}
		onchange={handleUpdateDefense}
		min={0}
		max={100}
	/>
</div>
