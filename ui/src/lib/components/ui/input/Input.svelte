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
		value = $bindable(''),
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
	/>
</label>
