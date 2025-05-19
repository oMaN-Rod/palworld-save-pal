<script lang="ts" generics="T extends Record<string, any> | string">
	import { cn } from '$theme';
	import type { Snippet } from 'svelte';
	import { Checkbox, Tooltip } from '$components/ui';

	let {
		items = $bindable([]),
		selectedItem = $bindable(),
		selectedItems = $bindable([]),
		selectAll = $bindable(false),
		baseClass: _baseClass = '',
		listClass: _listClass = '',
		itemClass: _itemClass = '',
		headerClass: _headerClass = 'grid-cols-[auto_55px_auto_1fr_auto]',
		multiple = true,
		canSelect = true,
		onlyHighlightChecked = false,
		listItem,
		listItemActions,
		listHeader,
		listItemPopup,
		onselect = (item: T) => {},
		idKey = 'id',
		...additionalProps
	} = $props<{
		items: T[];
		selectedItem?: T;
		selectedItems?: T[];
		selectAll?: boolean;
		baseClass?: string;
		listClass?: string;
		itemClass?: string;
		headerClass?: string;
		multiple?: boolean;
		canSelect?: boolean;
		onlyHighlightChecked?: boolean;
		listItem: Snippet<[T]>;
		listItemActions?: Snippet<[T]>;
		listHeader?: Snippet;
		listItemPopup?: Snippet<[T]>;
		onselect?: (item: T) => void;
		idKey?: string;
		[key: string]: any;
	}>();

	const baseClass = $derived(cn('flex flex-col', _baseClass));
	const listClass = $derived(
		cn(
			'list grow overflow-y-auto border-surface-900 border divide-y divide-surface-900',
			_listClass
		)
	);
	const itemClass = $derived(
		cn('list-item p-2 flex items-center cursor-pointer hover:bg-secondary-500/25', _itemClass)
	);
	const headerClass = $derived(cn('grid w-full gap-2', _headerClass));

	let selectedItemsSet: Set<string | number> = $state(new Set());

	function getItemKey(item: T): string | number {
		if (typeof item === 'string') {
			return item;
		}
		const key = (item as Record<string, any>)[idKey];
		return key as string | number;
	}

	$effect(() => {
		selectedItemsSet = new Set(selectedItems.map(getItemKey));
	});

	function processItem(item: T) {
		selectedItem = item;
		const key = getItemKey(item);

		if (multiple) {
			if (selectedItemsSet.has(key)) {
				selectedItemsSet.delete(key);
			} else {
				selectedItemsSet.add(key);
			}
			selectedItems = items.filter((i: T) => selectedItemsSet.has(getItemKey(i)));
		} else {
			selectedItems = [item];
			selectedItemsSet = new Set([key]);
		}
	}

	function handleCheckboxChange(item: T) {
		processItem(item);
	}

	function handleSelectAll() {
		selectedItem = null;
		if (selectAll) {
			selectedItems = items;
			selectedItemsSet = new Set(items.map(getItemKey));
		} else {
			selectedItems = [];
			selectedItemsSet.clear();
		}
	}

	function handleItemSelect(event: Event, item: T) {
		event.preventDefault();
		processItem(item);
		onselect(item);
	}

	function handleKeyDown(
		event: KeyboardEvent & { currentTarget: EventTarget & HTMLButtonElement },
		item: T
	) {
		if (event.key === 'Enter' || event.key === ' ') {
			handleItemSelect(event, item);
		}
	}

	function isSelected(item: T) {
		const key = getItemKey(item);
		return (!onlyHighlightChecked && item === selectedItem) || selectedItemsSet.has(key);
	}

	$effect(() => {
		if (items.length > 0) {
			selectAll = selectedItemsSet.size === items.length;
		} else {
			selectAll = false;
		}
	});
</script>

<div class={baseClass} {...additionalProps}>
	<div class="bg-surface-900 sticky top-0 z-10 shrink-0 p-2">
		<div class={headerClass}>
			{#if canSelect}
				<Checkbox bind:checked={selectAll} onchange={handleSelectAll} class="mr-2" />
			{/if}
			{@render listHeader?.()}
		</div>
	</div>
	<ul class={listClass}>
		{#each items as item (getItemKey(item))}
			<li class={cn(itemClass, isSelected(item) ? 'bg-secondary-500/25' : '')}>
				{#if canSelect}
					<Checkbox
						checked={selectedItemsSet.has(getItemKey(item))}
						onchange={() => handleCheckboxChange(item)}
						class="mr-2"
					/>
				{/if}
				<Tooltip
					background="bg-surface-700"
					baseClass="flex w-full items-center text-left"
					position="right"
				>
					<div class="flex w-full flex-row">
						<button
							class="flex w-full items-center text-left"
							onclick={(event) => handleItemSelect(event, item)}
							onkeydown={(event) => handleKeyDown(event, item)}
							aria-label={`Select item ${typeof item === 'string' ? item : ((item as Record<string, any>)[idKey] ?? '')}`}
						>
							{@render listItem(item)}
						</button>
						{@render listItemActions?.(item)}
					</div>
					{#snippet popup()}
						{@render listItemPopup(item)}
					{/snippet}
				</Tooltip>
			</li>
		{/each}
	</ul>
</div>
