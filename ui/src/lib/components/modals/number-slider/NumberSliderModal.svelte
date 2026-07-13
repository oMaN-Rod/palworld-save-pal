<script lang="ts">
	import { Button, Card, Input, Tooltip } from '$components/ui';
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
		markers,
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

	// Callers that pass explicit markers keep them; otherwise derive roughly ten evenly
	// spaced marks inside [min, max]. For the historical default (0-50) this yields
	// [5, 10, ..., 45] — the previous hard-coded list — but a small max (e.g. a relic
	// rank capped at 4) no longer draws markers beyond the end of the slider.
	const sliderMarkers = $derived.by(() => {
		if (markers) return markers;
		const span = max - min;
		if (span <= step) return [];
		const interval = Math.round(span / 10 / step) * step || step;
		const marks: number[] = [];
		for (let mark = min + interval; mark < max; mark += interval) marks.push(mark);
		return marks;
	});

	function handleClose(confirmed: boolean) {
		if (!confirmed) {
			closeModal(null);
			return;
		}
		// The number input lets you type past the slider's bounds -- and clearing it
		// yields NaN, which would sail straight through Math.min/Math.max. The modal
		// never returns a value outside [min, max].
		const raw = sliderValue[0];
		const value = typeof raw === 'number' && Number.isFinite(raw) ? raw : min;
		closeModal(Math.min(Math.max(value, min), max));
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
					markers={sliderMarkers}
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
					<Button variant="ghost" size="icon" onclick={() => handleClose(true)} data-modal-primary>
						<Save />
					</Button>
					{#snippet popup()}
						<span>{c.save}</span>
					{/snippet}
				</Tooltip>
				<Tooltip position="bottom">
					<Button variant="ghost" size="icon" onclick={() => handleClose(false)}>
						<X />
					</Button>
					{#snippet popup()}
						<span>{m.cancel()}</span>
					{/snippet}
				</Tooltip>
			</div>
		</div>
	</Card>
</div>
