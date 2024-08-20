<script lang="ts" generics="T">
	import type { SelectOption } from '$types';
	import { nanoid } from 'nanoid';
	import { cn } from '$theme';
	import type { Snippet } from 'svelte';
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
		selectOption,
		onChange = (value: string | number) => {},
		...additionalProps
	} = $props<{
		options: SelectOption[];
		selectClass?: string;
		labelClass?: string;
		labelTextClass?: string;
		label?: string;
		name?: string;
		value?: string | number;
		disabled?: boolean;
		error?: boolean;
		selectOption?: Snippet<[SelectOption]>;
		onChange?: (value: string | number) => void;
		[key: string]: any;
	}>();

	let selected = $state(typeof value === 'string' ? value : value.toString());
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

	const debounceSearch = debounce(searchOptions, 300);

	$effect(() => {
		if (value === 'None') {
			selected = '';
			searchTerm = '';
		} else {
			searchTerm = options.find((opt: SelectOption) => opt.value === value)?.label || '';
		}
		filteredOptions = options;
	});

	$effect(() => {
		value = selected;
		isOpen = false;
		onChange(selected);
	});

	$effect(() => {
		if (searchTerm) {
			debounceSearch();
		}
	});

	const selectClass = $derived(
		cn(
			'relative p-2 focus:outline-none ring-surface-200-800 focus-within:ring-secondary-500 ring rounded-sm',
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
			selected = option.value.toString();
			searchTerm = option.label;
			isOpen = false;
		}
	}

	function handleKeyDown(event: KeyboardEvent) {
		if (disabled) return;

		switch (event.key) {
			case 'Enter':
				event.preventDefault();
				if (isOpen) {
					const selectedOption = filteredOptions.find(
						(opt: SelectOption) => opt.value.toString() === selected
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
						(opt: SelectOption) => opt.value.toString() === selected
					);
					if (currentIndex < filteredOptions.length - 1) {
						selected = filteredOptions[currentIndex + 1].value.toString();
					}
				}
				break;
			case 'ArrowUp':
				event.preventDefault();
				if (isOpen) {
					// Move to previous option
					const currentIndex = filteredOptions.findIndex(
						(opt: SelectOption) => opt.value.toString() === selected
					);
					if (currentIndex > 0) {
						selected = filteredOptions[currentIndex - 1].value.toString();
					}
				}
				break;
		}
	}

	$effect(() => {
		const handleClickOutside = (event: MouseEvent) => {
			if (containerRef && !containerRef.contains(event.target as Node)) {
				isOpen = false;
			}
		};

		document.addEventListener('click', handleClickOutside);

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
				class="w-full bg-transparent focus:outline-none"
				placeholder="Search..."
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
				class="bg-surface-800 border-surface-600 absolute left-0 right-0 z-10 mt-1 max-h-60 overflow-auto rounded-sm border shadow-lg"
				role="listbox"
			>
				{#each filteredOptions as option}
					<div
						class={cn(
							'hover:bg-surface-700 cursor-pointer p-2',
							option.value.toString() === selected && 'bg-primary-500'
						)}
						role="option"
						tabindex={0}
						aria-selected={option.value.toString() === selected}
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
