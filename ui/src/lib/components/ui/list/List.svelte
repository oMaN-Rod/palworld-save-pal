<script lang="ts" generics="T">
	import { cn } from '$theme';
	import type { Snippet } from 'svelte';
	import { Checkbox, Tooltip } from '$components/ui';
	import { nanoid } from 'nanoid';

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
		idKey = 'id'
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
		onselect?: (item: any) => void;
		idKey?: string;
	}>();

	const baseClass = $derived(cn('flex flex-col', _baseClass));
	const listClass = $derived(
		cn(
			'list flex-grow overflow-y-auto border-surface-900 border divide-y divide-surface-900',
			_listClass
		)
	);
	const itemClass = $derived(
		cn('list-item p-2 flex items-center cursor-pointer hover:bg-secondary-500/25', _itemClass)
	);
	const headerClass = $derived(cn('grid w-full gap-2', _headerClass));
	let selectedItemsSet: Set<any> = $state(new Set());

	$effect(() => {
		selectedItemsSet = new Set(selectedItems.map((item: { [x: string]: any }) => item[idKey]));
	});

	function processItem(item: any) {
		selectedItem = item;
		if (multiple) {
			if (selectedItemsSet.has(item[idKey])) {
				selectedItemsSet.delete(item[idKey]);
			} else {
				selectedItemsSet.add(item[idKey]);
			}
			selectedItems = items.filter((i: { [x: string]: any }) => selectedItemsSet.has(i[idKey]));
		} else {
			selectedItems = [item];
			selectedItemsSet = new Set([item[idKey]]);
		}
	}

	function handleCheckboxChange(item: any) {
		processItem(item);
	}

	function handleSelectAll() {
		selectedItem = null;
		if (selectAll) {
			selectedItems = items;
			selectedItemsSet = new Set(items.map((item: { [x: string]: any }) => item[idKey]));
		} else {
			selectedItems = [];
			selectedItemsSet.clear();
		}
	}

	function handleItemSelect(event: Event, item: any) {
		event.preventDefault();
		selectedItem = item;
		onselect(selectedItem);
	}

	function handleKeyDown(
		event: KeyboardEvent & { currentTarget: EventTarget & HTMLButtonElement },
		item: any
	): any {
		if (event.key === 'Enter' || event.key === ' ') {
			handleItemSelect(event, item);
		}
	}

	function isSelected(item: any) {
		return (!onlyHighlightChecked && item === selectedItem) || selectedItemsSet.has(item[idKey]);
	}

	$effect(() => {
		selectAll = items.length > 0 && selectedItemsSet.size === items.length;
	});
</script>

<div class={baseClass}>
	<div class="bg-surface-900 sticky top-0 z-10 flex-shrink-0 p-2">
		<div class={headerClass}>
			{#if canSelect}
				<Checkbox bind:checked={selectAll} onchange={handleSelectAll} class="mr-2" />
			{/if}
			{@render listHeader()}
		</div>
	</div>
	<ul class={listClass}>
		{#each items as item (item[idKey])}
			<li class={cn(itemClass, isSelected(item) ? 'bg-secondary-500/25' : '')}>
				{#if canSelect}
					<Checkbox
						checked={selectedItemsSet.has(item[idKey])}
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
						>
							{@render listItem(item)}
						</button>
						{#if listItemActions}
							{@render listItemActions(item)}
						{/if}
					</div>
					{#snippet popup()}
						{@render listItemPopup(item)}
					{/snippet}
				</Tooltip>
			</li>
		{/each}
	</ul>
</div>
