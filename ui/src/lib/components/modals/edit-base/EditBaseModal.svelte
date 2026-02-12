<script lang="ts">
	import { Card, Input, Tooltip } from '$components/ui';
	import { Save, X } from 'lucide-svelte';
	import { Slider } from '@skeletonlabs/skeleton-svelte';
	import type { ValueChangeDetails } from '@zag-js/slider';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let {
		title = '',
		name = $bindable(''),
		areaRange = $bindable(3500),
		closeModal
	} = $props<{
		title?: string;
		name?: string;
		areaRange?: number;
		closeModal: (value: any) => void;
	}>();

	let sliderValue: number[] = $state([areaRange]);
	let modalContainer: HTMLDivElement;
	const markers = Array.from({ length: 10 }, (_, i) => 1750 + i * 3500);

	function handleClose(confirmed: boolean) {
		closeModal(confirmed ? { name, area_range: sliderValue[0] } : null);
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-[calc(100vw/3)]">
		<h3 class="h3">{title}</h3>

		<div class="mt-2 flex flex-col gap-2">
			<Input inputClass="grow" bind:value={name} label={m.name()} />
			<span class="label-text">{m.base_range()}</span>
			<div class="flex w-full items-center">
				<Slider
					classes="w-10/12 mr-2"
					value={sliderValue}
					min={1750}
					max={35000}
					{markers}
					step={250}
					height="h-2"
					meterBg="bg-secondary-500"
					thumbRingColor="ring-secondary-500"
					onValueChange={(e: ValueChangeDetails) => (sliderValue[0] = e.value[0])}
				/>
				<Input
					labelClass="w-1/4"
					type="number"
					bind:value={sliderValue[0]}
					min={1750}
					max={35000}
				/>
			</div>
			<div class="mt-2 flex justify-end">
				<Tooltip position="bottom">
					<button
						class="btn hover:bg-secondary-500 px-2"
						onclick={() => handleClose(true)}
						data-modal-primary
					>
						<Save />
					</button>
					{#snippet popup()}
						<span>{c.save}</span>
					{/snippet}
				</Tooltip>
				<Tooltip position="bottom">
					<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(false)}>
						<X />
					</button>
					{#snippet popup()}
						<span>{m.cancel()}</span>
					{/snippet}
				</Tooltip>
			</div>
		</div>
	</Card>
</div>
