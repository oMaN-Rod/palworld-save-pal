<script lang="ts" generics="T">
	import { cn } from '$theme';
	import type { Snippet } from 'svelte';
	import { Checkbox, Tooltip } from '$components/ui';

	let {
		items = $bindable([]),
		selectedItem = $bindable(),
		selectedItems = $bindable({}),
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
		onselect = (item: T) => {}
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

	function processItem(item: any) {
		selectedItem = item;
		if (multiple) {
			selectedItems.push(item);
		} else {
			selectedItems = [];
		}
	}

	function handleCheckboxChange(item: any, event: Event) {
		const isChecked = (event.target as HTMLInputElement).checked;
		if (isChecked) {
			processItem(item);
		} else {
			selectedItems = selectedItems.filter((selectedItem: any) => selectedItem !== item);
		}
	}

	function handleSelectAll() {
		selectedItem = null;
		selectedItems = [];
		if (selectAll) {
			selectedItems = items;
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
		return (
			(JSON.stringify(item) == JSON.stringify(selectedItem) && !onlyHighlightChecked) ||
			(Array.isArray(selectedItems) &&
				selectedItems.length > 0 &&
				selectedItems.some((i: any) => JSON.stringify(i) == JSON.stringify(item)))
		);
	}
</script>

<div class={baseClass}>
	<div class="bg-surface-900 sticky top-0 z-10 flex-shrink-0 p-2">
		<div class={headerClass}>
			{#if canSelect}
				<Checkbox checked={selectAll} onchange={handleSelectAll} class="mr-2" />
			{/if}
			{@render listHeader()}
		</div>
	</div>
	<ul class={listClass}>
		{#each items as item}
			<li class={cn(itemClass, isSelected(item) ? 'bg-secondary-500/25' : '')}>
				{#if canSelect}
					<Checkbox
						checked={isSelected(item)}
						onchange={(event) => handleCheckboxChange(item, event)}
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
