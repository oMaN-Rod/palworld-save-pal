<script lang="ts" generics="T">
	import { cn } from '$theme';
	import type { Snippet } from 'svelte';
	import { Checkbox, Tooltip } from '$components/ui';

	let {
		items = $bindable([]),
		selectedItem = $bindable({}),
		selectedItems = $bindable({}),
		baseClass: _baseClass = '',
		listClass: _listClass = '',
		itemClass: _itemClass = '',
		headerClass: _headerClass = 'grid-cols-[auto_55px_auto_1fr_auto]',
		multiple = true,
		listItem,
		listItemActions,
		listHeader,
		listItemPopup,
		onselect = (item: T) => {}
	} = $props<{
		items: T[];
		selectedItem?: T;
		selectedItems?: T[];
		baseClass?: string;
		listClass?: string;
		itemClass?: string;
		headerClass?: string;
		multiple?: boolean;
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

	let selectAll: boolean = $state(false);

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
		console.log('selectAll', selectAll);
		selectedItem = {};
		selectedItems = [];
		if (selectAll) {
			selectedItems = items;
		}
		console.log('selectedItems', JSON.stringify(selectedItems, null, 2));
		console.log('items', JSON.stringify(items, null, 2));
	}

	function handleItemSelect(event: Event, item: any) {
		onselect(item);
		selectedItem = item;
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
		return item === selectedItem || selectedItems.includes(item);
	}
</script>

<div class={baseClass}>
	<div class="bg-surface-900 sticky top-0 z-10 flex-shrink-0 p-2">
		<div class={headerClass}>
			<Checkbox bind:checked={selectAll} onchange={handleSelectAll} />
			{@render listHeader()}
		</div>
	</div>
	<ul class={listClass}>
		{#each items as item}
			<li class={cn(itemClass, isSelected(item) ? 'bg-secondary-500/25' : '')}>
				<Checkbox
					checked={selectedItems.includes(item)}
					onchange={(event) => handleCheckboxChange(item, event)}
					class="mr-2"
				/>
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
