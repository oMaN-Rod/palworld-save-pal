<script lang="ts">
	import { cn } from '$theme';
	import { nanoid } from 'nanoid';

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
		value: _value = $bindable(''),
		disabled = false,
		name = nanoid(),
		autocomplete = undefined,
		error = false,
		step = undefined,
		min = undefined,
		max = undefined,
		format = 'text',
		...additionalProps
	} = $props<{
		type?: InputType;
		inputClass?: string;
		labelClass?: string;
		labelTextClass?: string;
		placeholder?: string;
		label?: string;
		value?: string | number;
		disabled?: boolean;
		name?: string;
		autocomplete?: string | null | undefined;
		error?: boolean;
		step?: number | undefined;
		min?: number | undefined;
		max?: number | undefined;
		format?: FormatAs;
		[key: string]: any;
	}>();

	let formattedValue = $state(formatValue(_value.toString()));

	$effect(() => {
		formattedValue = formatValue(formattedValue);
		_value = formattedValue.replace(/,/g, '');
	});

	const inputClass = $derived(
		cn(
			'input p-2 my-2 focus:outline-none ring-surface-200-800 focus-within:ring-secondary-500 ring rounded-sm bg-surface-800',
			error ? 'border-error' : '',
			disabled ? 'text-gray-400 cursor-not-allowed' : '',
			_inputClass
		)
	);

	const labelClass = $derived(cn('label', _labelClass));

	const labelTextClass = $derived(cn('label-text', _labelTextClass));

	function formatValue(value: string) {
		if (format === 'text') return value;
		if (format === 'currency') {
			const numericValue = value.replace(/,/g, '');
			const formattedNumber = parseFloat(numericValue)
				.toLocaleString('en-US', {
					style: 'currency',
					currency: 'USD',
					minimumFractionDigits: 2
				})
				.replace('$', '');
			return formattedNumber;
		} else {
			const numericValue = value.replace(/,/g, '');
			const formattedNumber = numericValue.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
			return formattedNumber;
		}
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
		bind:value={formattedValue}
		{disabled}
		{autocomplete}
		class={inputClass}
		{...additionalProps}
	/>
</label>
