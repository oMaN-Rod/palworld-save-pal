<script lang="ts">
	import type { SelectOption } from '$types';
	import { nanoid } from 'nanoid';
	import { cn } from '$theme';
	import { onMount, type Snippet } from 'svelte';
	import { ChevronDown } from 'lucide-svelte';
	import { debounce } from '$utils';

	let {
		options = [],
		selectClass: _selectClass = 'bg-surface-900',
		labelClass: _labelClass = '',
		labelTextClass: _labelTextClass = '',
		label = '',
		name = nanoid(),
		value = $bindable(''),
		disabled = false,
		error = false,
		placeholder = 'Search...',
		selectOption,
		onChange = () => {},
		...additionalProps
	}: {
		options: SelectOption[];
		selectClass?: string;
		labelClass?: string;
		labelTextClass?: string;
		label?: string;
		name?: string;
		value?: string | number;
		disabled?: boolean;
		error?: boolean;
		placeholder?: string;
		selectOption?: Snippet<[SelectOption]>;
		onChange?: (value: string | number) => void;
		[key: string]: any;
	} = $props();

	let isOpen = $state(false);
	let containerRef: HTMLDivElement;
	let listboxId = nanoid();
	let searchTerm = $state('');
	let filteredOptions = $state(options);

	async function searchOptions() {
		filteredOptions = options.filter((option: SelectOption) =>
			option.label.toLowerCase().includes(searchTerm.toLowerCase())
		);
	}

	const debounceSearch = debounce(searchOptions, 200);

	const selectClass = $derived(
		cn(
			'relative p-2 focus:outline-hidden ring-surface-200-800 focus-within:ring-secondary-500 ring rounded-xs',
			error ? 'border-error' : '',
			disabled ? 'text-gray-400 cursor-not-allowed' : '',
			_selectClass
		)
	);

	const labelClass = $derived(
		cn('label my-2', error ? 'text-error' : '', disabled ? 'text-gray-400' : '', _labelClass)
	);

	const labelTextClass = $derived(
		cn('label-text', disabled ? 'text-gray-400' : '', _labelTextClass)
	);

	function handleOptionClick(option: SelectOption) {
		if (!disabled) {
			value = option.value.toString();
			searchTerm = option.label;
			isOpen = false;
			onChange(value);
		}
	}

	function handleKeyDown(event: KeyboardEvent) {
		if (disabled) return;

		switch (event.key) {
			case 'Enter':
				event.preventDefault();
				if (isOpen) {
					const selectedOption = filteredOptions.find(
						(opt: SelectOption) => opt.value.toString() === value
					);
					if (selectedOption) {
						handleOptionClick(selectedOption);
					}
				} else {
					isOpen = true;
				}
				break;
			case 'Escape':
				isOpen = false;
				break;
			case 'ArrowDown':
				event.preventDefault();
				if (!isOpen) {
					isOpen = true;
				} else {
					// Move to next option
					const currentIndex = filteredOptions.findIndex(
						(opt: SelectOption) => opt.value.toString() === value
					);
					if (currentIndex < filteredOptions.length - 1) {
						value = filteredOptions[currentIndex + 1].value.toString();
					}
				}
				break;
			case 'ArrowUp':
				event.preventDefault();
				if (isOpen) {
					// Move to previous option
					const currentIndex = filteredOptions.findIndex(
						(opt: SelectOption) => opt.value.toString() === value
					);
					if (currentIndex > 0) {
						value = filteredOptions[currentIndex - 1].value.toString();
					}
				}
				break;
		}
	}

	$effect(() => {
		if (searchTerm) {
			debounceSearch();
		} else {
			filteredOptions = options;
		}
	});

	onMount(() => {
		const handleClickOutside = (event: MouseEvent) => {
			if (containerRef && !containerRef.contains(event.target as Node)) {
				isOpen = false;
			}
		};

		document.addEventListener('click', handleClickOutside);

		if (value !== 'None') {
			searchTerm = options.find((opt: SelectOption) => opt.value === value)?.label || '';
		}
		filteredOptions = options;

		return () => {
			document.removeEventListener('click', handleClickOutside);
		};
	});
</script>

<div class={labelClass} bind:this={containerRef}>
	{#if label}
		<span class={labelTextClass}>
			{label}
		</span>
	{/if}
	<div
		class={selectClass}
		tabindex={disabled ? -1 : 0}
		onkeydown={handleKeyDown}
		role="combobox"
		aria-expanded={isOpen}
		aria-haspopup="listbox"
		aria-controls={listboxId}
		aria-labelledby={label}
		{...additionalProps}
	>
		<div class="flex items-center justify-between">
			<input
				type="text"
				class="focus:outline-hidden w-full bg-transparent"
				{placeholder}
				bind:value={searchTerm}
				onfocus={() => (isOpen = true)}
				oninput={() => (isOpen = true)}
			/>
			<ChevronDown
				class={cn('cursor-pointer transition-transform', isOpen && 'rotate-180')}
				onclick={() => (isOpen = !isOpen)}
			/>
		</div>
		{#if isOpen}
			<div
				id={listboxId}
				class="bg-surface-900 border-surface-600 select-popup rounded-xs absolute left-0 right-0 mt-3 max-h-60 overflow-auto border shadow-lg"
				role="listbox"
			>
				{#each filteredOptions as option}
					<div
						class={cn(
							'hover:bg-surface-700 cursor-pointer p-2',
							option.value.toString() === value && 'bg-primary-500'
						)}
						role="option"
						tabindex={0}
						aria-selected={option.value.toString() === value}
						onclick={() => handleOptionClick(option)}
						onkeydown={(e) => {
							if (e.key === 'Enter' || e.key === ' ') {
								e.preventDefault();
								handleOptionClick(option);
							}
						}}
					>
						{#if selectOption}
							{@render selectOption(option)}
						{:else}
							{option.label}
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>

<style>
	.select-popup {
		z-index: 99999;
	}
</style>
