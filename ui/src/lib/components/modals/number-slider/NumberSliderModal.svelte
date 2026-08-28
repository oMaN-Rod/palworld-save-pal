<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Card, Input, Slider, Tooltip } from '$components/ui';
	import { onMount, untrack } from 'svelte';
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

	let sliderValue: number = $state(untrack(() => value));
	let modalContainer: HTMLDivElement;

	// Derives roughly ten evenly spaced marks inside [min, max] when the caller doesn't pass explicit markers,
	// so a small max (e.g. a relic rank capped at 4) doesn't draw markers beyond the end of the slider.
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
		// Clearing the number input yields NaN, which would sail straight through Math.min/Math.max.
		const raw = sliderValue;
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
					class="mr-2 w-10/12"
					bind:value={sliderValue}
					{min}
					{max}
					{step}
					markers={sliderMarkers}
					label={title}
				/>
				<Input labelClass="w-2/12" type="number" bind:value={sliderValue} {min} {max} />
			</div>
			<div class="flex w-full justify-end">
				<Tooltip position="bottom">
					<Button variant="ghost" size="icon" onclick={() => handleClose(true)} data-modal-primary>
						<Icon icon="tabler:device-floppy" />
					</Button>
					{#snippet popup()}
						<span>{c.save}</span>
					{/snippet}
				</Tooltip>
				<Tooltip position="bottom">
					<Button variant="ghost" size="icon" onclick={() => handleClose(false)}>
						<Icon icon="tabler:x" />
					</Button>
					{#snippet popup()}
						<span>{m.cancel()}</span>
					{/snippet}
				</Tooltip>
			</div>
		</div>
	</Card>
</div>
