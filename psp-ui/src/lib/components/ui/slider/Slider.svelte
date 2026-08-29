<script lang="ts">
	import { cn } from '$theme';
	import { coarseStep, fractionOf, quantize, valueFromFraction } from './slider';
	import type { SliderSize } from './slider';

	type SliderColor =
		| 'primary'
		| 'secondary'
		| 'tertiary'
		| 'success'
		| 'warning'
		| 'error'
		| 'surface';

	let {
		value = $bindable(0),
		min = 0,
		max = 100,
		step = 1,
		disabled = false,
		size = 'md',
		thumb,
		showSteppers,
		showValue,
		showMax = false,
		markers,
		markerStyle = 'labels',
		color = 'secondary',
		completeColor,
		format,
		label,
		onchange,
		class: className = ''
	}: {
		value?: number;
		min?: number;
		max?: number;
		step?: number;
		disabled?: boolean;
		size?: SliderSize;
		thumb?: boolean;
		showSteppers?: boolean;
		showValue?: boolean;
		showMax?: boolean;
		markers?: number[];
		markerStyle?: 'labels' | 'ticks';
		color?: SliderColor;
		completeColor?: SliderColor;
		format?: (value: number) => string;
		label?: string;
		onchange?: (value: number) => void;
		class?: string;
	} = $props();

	const FILL_CLASS: Record<SliderColor, string> = {
		primary: 'bg-primary-500',
		secondary: 'bg-secondary-500',
		tertiary: 'bg-tertiary-500',
		success: 'bg-success-500',
		warning: 'bg-warning-500',
		error: 'bg-error-500',
		surface: 'bg-surface-500'
	};

	const SIZE_CLASS = {
		xs: {
			track: 'h-1.5 rounded-full border-0',
			value: 'text-[10px]',
			stepper: 'h-4 w-4 text-[10px]',
			max: 'text-[9px]',
			marker: 'text-[9px]',
			markerPad: 'pb-3.5',
			thumb: 'h-3 w-3',
			thumbPx: 12
		},
		sm: {
			track: 'h-6',
			value: 'text-[10px]',
			stepper: 'h-5 w-5 text-xs',
			max: 'text-[9px]',
			marker: 'text-[10px]',
			markerPad: 'pb-4',
			thumb: 'h-4 w-4',
			thumbPx: 16
		},
		md: {
			track: 'h-7',
			value: 'text-[11px]',
			stepper: 'h-6 w-6 text-sm',
			max: 'text-[10px]',
			marker: 'text-[10px]',
			markerPad: 'pb-4',
			thumb: 'h-4.5 w-4.5',
			thumbPx: 18
		}
	} satisfies Record<SliderSize, Record<string, string | number>>;

	let track: HTMLDivElement | undefined = $state();
	// Dragging suppresses the width transition: an animated fill lags the
	// pointer, which reads as the bar fighting the drag.
	let dragging = $state(false);

	let current = $derived(quantize(value, min, max, step));
	let fraction = $derived(fractionOf(current, min, max));
	let complete = $derived(current >= max);
	let sizes = $derived(SIZE_CLASS[size]);
	// A rail this thin cannot hold a value label, and steppers beside it would
	// tower over the track, so xs defaults to a bare rail with a thumb.
	let compact = $derived(size === 'xs');
	let hasThumb = $derived(thumb ?? compact);
	let hasSteppers = $derived(showSteppers ?? !compact);
	let hasValue = $derived(showValue ?? !compact);
	let fillClass = $derived(FILL_CLASS[complete && completeColor ? completeColor : color]);
	let hasTicks = $derived((markers?.length ?? 0) > 0 && markerStyle === 'ticks');
	let hasLabels = $derived((markers?.length ?? 0) > 0 && markerStyle === 'labels');
	// leading-none trails the size class: the size sets a line-height too, and
	// tailwind-merge keeps only the last of the pair.
	let stepperClass = $derived(
		cn(
			'border-surface-200 dark:border-surface-700 bg-surface-100 dark:bg-surface-900 text-surface-400 hover:bg-surface-200 dark:hover:bg-surface-800 flex shrink-0 items-center justify-center rounded-sm border transition-colors disabled:opacity-30',
			sizes.stepper,
			'leading-none'
		)
	);
	let display = $derived(format ? format(current) : String(current));

	function commit(next: number): void {
		if (next === current) return;
		value = next;
		onchange?.(next);
	}

	function valueAt(clientX: number): number {
		if (!track) return current;
		const rect = track.getBoundingClientRect();
		if (rect.width === 0) return current;
		return valueFromFraction((clientX - rect.left) / rect.width, min, max, step);
	}

	function bump(delta: number): void {
		commit(quantize(current + delta, min, max, step));
	}

	function onpointerdown(e: PointerEvent): void {
		if (disabled || e.button !== 0) return;
		dragging = true;
		// Capture keeps the drag alive once the pointer leaves the track.
		track?.setPointerCapture(e.pointerId);
		commit(valueAt(e.clientX));
	}

	function onpointermove(e: PointerEvent): void {
		if (!dragging || disabled) return;
		e.preventDefault();
		commit(valueAt(e.clientX));
	}

	function endDrag(): void {
		dragging = false;
	}

	// Arrows step ±1 (±5% of the track with Shift), Home/End snap to the ends.
	function onkeydown(e: KeyboardEvent): void {
		if (disabled) return;
		const delta = e.shiftKey ? coarseStep(min, max, step) : step;
		let next: number | undefined;
		switch (e.key) {
			case 'ArrowLeft':
			case 'ArrowDown':
				next = quantize(current - delta, min, max, step);
				break;
			case 'ArrowRight':
			case 'ArrowUp':
				next = quantize(current + delta, min, max, step);
				break;
			case 'Home':
				next = min;
				break;
			case 'End':
				next = max;
				break;
		}
		if (next === undefined) return;
		e.preventDefault();
		commit(next);
	}
</script>

<div class={cn('flex items-center gap-1.5', hasLabels && sizes.markerPad, className)}>
	{#if hasSteppers}
		<button
			type="button"
			class={stepperClass}
			onclick={() => bump(-step)}
			disabled={disabled || current <= min}
			aria-label={label ? `${label} −` : undefined}
		>
			−
		</button>
	{/if}

	<div
		bind:this={track}
		class={cn(
			'border-surface-200 dark:border-surface-700 bg-surface-100 dark:bg-surface-900 relative flex-1 touch-none rounded-sm border',
			disabled ? 'cursor-not-allowed opacity-60' : 'cursor-pointer',
			sizes.track
		)}
		role="slider"
		tabindex={disabled ? -1 : 0}
		aria-label={label}
		aria-valuemin={min}
		aria-valuemax={max}
		aria-valuenow={current}
		aria-valuetext={format ? display : undefined}
		aria-disabled={disabled}
		{onkeydown}
		{onpointerdown}
		{onpointermove}
		onpointerup={endDrag}
		onpointercancel={endDrag}
	>
		<!-- The fill is clipped by its own layer so the thumb, which is taller
		     than a compact track, is free to overflow it. -->
		<div class="absolute inset-0 overflow-hidden rounded-[inherit]">
			<div
				class={cn(
					'h-full rounded-[inherit]',
					dragging ? '' : 'transition-all duration-200',
					fillClass
				)}
				style:width="{fraction * 100}%"
			></div>
			{#if hasTicks}
				{#each markers ?? [] as mark (mark)}
					<span
						class="bg-surface-400/60 pointer-events-none absolute inset-y-0 w-px"
						style:left="{fractionOf(mark, min, max) * 100}%"
					></span>
				{/each}
			{/if}
		</div>
		{#if hasThumb}
			<div
				class={cn(
					'pointer-events-none absolute top-1/2 -translate-y-1/2 rounded-full border-2 border-white/90 shadow-sm',
					dragging ? '' : 'transition-all duration-200',
					sizes.thumb,
					fillClass
				)}
				style:left="calc({fraction} * (100% - {sizes.thumbPx}px))"
			></div>
		{/if}
		{#if hasValue}
			<span
				class={cn(
					'pointer-events-none absolute inset-0 flex items-center justify-center font-semibold text-white tabular-nums',
					sizes.value
				)}
			>
				{display}
			</span>
		{/if}
		{#if hasLabels}
			{#each markers ?? [] as mark (mark)}
				<span
					class={cn(
						'text-surface-400 pointer-events-none absolute top-full mt-0.5 -translate-x-1/2 tabular-nums',
						sizes.marker
					)}
					style:left="{fractionOf(mark, min, max) * 100}%"
				>
					{format ? format(mark) : mark}
				</span>
			{/each}
		{/if}
	</div>

	{#if hasSteppers}
		<button
			type="button"
			class={stepperClass}
			onclick={() => bump(step)}
			disabled={disabled || current >= max}
			aria-label={label ? `${label} +` : undefined}
		>
			+
		</button>
	{/if}

	{#if showMax}
		<span class={cn('text-surface-400 w-8 shrink-0 text-right tabular-nums', sizes.max)}>
			/{max}
		</span>
	{/if}
</div>
