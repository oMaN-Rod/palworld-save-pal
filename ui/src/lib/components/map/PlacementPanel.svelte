<script lang="ts">
	import { Button, Checkbox, Select } from '$components/ui';
	import { Slider } from '@skeletonlabs/skeleton-svelte';
	import type { ValueChangeDetails } from '@zag-js/slider';
	import { placementState } from '$lib/data/placement.svelte';
	import type { SelectOption } from '$types';

	let {
		guildOptions,
		playerOptions,
		onPlace,
		onCancel
	}: {
		guildOptions: SelectOption[];
		playerOptions: SelectOption[];
		onPlace: () => void;
		onCancel: () => void;
	} = $props();

	const anchor = $derived(placementState.anchor);

	// Derived (not seeded once) so the sliders reflect the origin default applied
	// after geometry loads, and any later anchor change.
	const yawDeg = $derived([(anchor.yaw * 180) / Math.PI]);
	const zCm = $derived([anchor.z]);

	function handleYawChange(e: ValueChangeDetails) {
		placementState.setAnchor({ ...anchor, yaw: (e.value[0] * Math.PI) / 180 });
	}

	function handleZChange(e: ValueChangeDetails) {
		placementState.setAnchor({ ...anchor, z: e.value[0] });
	}

	function setZ(z: number) {
		if (!Number.isNaN(z)) placementState.setAnchor({ ...anchor, z });
	}

	const placeDisabled = $derived(
		placementState.hasBlocking ||
			!placementState.targetGuild ||
			!placementState.targetPlayer ||
			(placementState.findings.length > 0 && !placementState.overrideWarnings)
	);
</script>

<aside
	class="bg-surface-900/95 absolute top-2 right-14 bottom-2 z-10 flex h-[calc(100vh-80px)] w-90 flex-col gap-4 overflow-y-auto rounded-lg p-4 shadow-lg"
>
	<div class="flex flex-col gap-1">
		<h2 class="text-lg font-bold">Place Blueprint</h2>
		<p class="text-surface-400 text-sm">{placementState.header?.name ?? ''}</p>
	</div>

	<Select
		label="Guild"
		options={guildOptions}
		value={placementState.targetGuild}
		placeholder="Select a guild"
		onChange={(v) => (placementState.targetGuild = v.toString())}
	/>

	<Select
		label="Player"
		options={playerOptions}
		value={placementState.targetPlayer}
		placeholder="Select a player"
		onChange={(v) => (placementState.targetPlayer = v.toString())}
	/>

	<div class="flex flex-col gap-2">
		<span class="label-text">Rotation (yaw): {Math.round(yawDeg[0])}°</span>
		<Slider value={yawDeg} min={0} max={360} step={1} onValueChange={handleYawChange} />
	</div>

	<div class="flex flex-col gap-2">
		<div class="flex items-center justify-between gap-2">
			<span class="label-text">Height (Z)</span>
			<input
				type="number"
				step="1"
				value={Math.round(anchor.z)}
				oninput={(e) => setZ(Number((e.currentTarget as HTMLInputElement).value))}
				class="input w-28 text-right"
				aria-label="Height Z in centimeters"
			/>
			<span class="label-text">cm</span>
		</div>
		<Slider value={zCm} min={-50000} max={50000} step={10} onValueChange={handleZChange} />
	</div>

	<Checkbox label="Override warnings" bind:checked={placementState.overrideWarnings} />
	<p class="text-surface-500 text-xs">Allows placement to proceed despite non-blocking warnings.</p>

	{#if placementState.findings.length > 0}
		<div class="flex flex-col gap-1">
			<span class="label-text">Findings</span>
			<ul class="flex max-h-36 flex-col gap-1 overflow-y-auto 2xl:max-h-90">
				{#each placementState.findings as finding, i (i)}
					<li
						class="rounded-sm border p-2 text-xs {finding.severity === 'blocking'
							? 'border-error-500 bg-error-500/10 text-error-400'
							: 'border-warning-500 bg-warning-500/10 text-warning-400'}"
					>
						{finding.message}
					</li>
				{/each}
			</ul>
		</div>
	{/if}

	<div class="mt-auto flex justify-end gap-2">
		<Button variant="neutral" onclick={onCancel}>Cancel</Button>
		<Button variant="primary" disabled={placeDisabled} onclick={onPlace}>Place</Button>
	</div>
</aside>
