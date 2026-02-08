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
		value = 0,
		markers = [5, 10, 15, 20, 25, 30, 35, 40, 45],
		step = 1,
		min = 0,
		max = 50,
		closeModal
	} = $props<{
		title?: string;
		value?: number;
		markers?: number[];
		step?: number;
		min?: number;
		max?: number;
		closeModal: (value: any) => void;
	}>();

	let sliderValue: number[] = $state([value]);
	let modalContainer: HTMLDivElement;

	function handleClose(confirmed: boolean) {
		closeModal(confirmed ? sliderValue[0] : null);
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-[calc(100vw/3)]">
		<h3 class="h3">{title}</h3>

		<div class="mt-2 flex flex-col items-center space-x-2">
			<div class="flex w-full items-center">
				<Slider
					classes="w-10/12 mr-2"
					value={sliderValue}
					{min}
					{max}
					{markers}
					{step}
					height="h-2"
					meterBg="bg-secondary-500"
					thumbRingColor="ring-secondary-500"
					onValueChange={(e: ValueChangeDetails) => (sliderValue[0] = e.value[0])}
				/>
				<Input labelClass="w-2/12" type="number" bind:value={sliderValue[0]} {min} {max} />
			</div>
			<div class="flex w-full justify-end">
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
