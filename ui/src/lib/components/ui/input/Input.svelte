<script lang="ts" generics="T">
	import { cn } from '$theme';
	import { nanoid } from 'nanoid';
	import type { FullAutoFill } from 'svelte/elements';

	type FormatAs = 'text' | 'currency' | 'number';
	type InputType =
		| 'text'
		| 'number'
		| 'email'
		| 'password'
		| 'search'
		| 'date'
		| 'time'
		| 'tel'
		| 'url';

	let {
		type = 'text',
		inputClass: _inputClass = '',
		labelClass: _labelClass = '',
		labelTextClass: _labelTextClass = '',
		placeholder = '',
		label = '',
		value = $bindable('' as unknown as T),
		disabled = false,
		name = nanoid(),
		autocomplete = undefined,
		error = false,
		step = undefined,
		min = undefined,
		max = undefined,
		format = 'text',
		onValueChange = (newValue: T) => {},
		...additionalProps
	}: {
		type?: InputType;
		inputClass?: string;
		labelClass?: string;
		labelTextClass?: string;
		placeholder?: string;
		label?: string;
		value?: T;
		disabled?: boolean;
		name?: string;
		autocomplete?: FullAutoFill | null | undefined;
		error?: boolean;
		step?: number | undefined;
		min?: number;
		max?: number;
		format?: FormatAs;
		onValueChange?: (newValue: T) => void;
		[key: string]: any;
	} = $props();

	const inputClass = $derived(
		cn(
			'input p-2 my-2 focus:outline-hidden ring-surface-200-800 focus-within:ring-secondary-500 ring rounded-xs bg-surface-900',
			error ? 'border-error' : '',
			disabled ? 'text-gray-400 cursor-not-allowed' : '',
			_inputClass
		)
	);

	const labelClass = $derived(cn('label', _labelClass));

	const labelTextClass = $derived(cn('label-text', _labelTextClass));

	function handleValueChange() {
		if (typeof value === 'number' && max !== undefined && min !== undefined) {
			const clampedValue = value > max ? max : value < min ? min : value;
			value = clampedValue as T;
		}
		onValueChange(value);
	}
</script>

<label class={labelClass}>
	{#if label}
		<span class={labelTextClass}>
			{label}
		</span>
	{/if}
	<input
		{name}
		{type}
		{step}
		{min}
		{max}
		{placeholder}
		bind:value
		{disabled}
		{autocomplete}
		class={inputClass}
		{...additionalProps}
		onchange={handleValueChange}
	/>
</label>
