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
		itemsKey = 'instance_id',
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
		itemsKey?: string;
		multiple?: boolean;
		listItem: Snippet<[T]>;
		listItemActions?: Snippet<[T]>;
		listHeader?: Snippet;
		listItemPopup?: Snippet<[T]>;
		onselect?: (item: any) => void;
	}>();

	const baseClass = $derived(cn('h-[calc(100vh-200px)] overflow-hidden', _baseClass));
	const listClass = $derived(
		cn(
			'list w-full h-full border-surface-900 border divide-y divide-surface-900 overflow-y-auto',
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

	function handleCheckboxChange(item: any, isChecked: boolean) {
		if (isChecked) {
			processItem(item);
		} else {
			selectedItems = selectedItems.filter((selectedItem: any) => selectedItem !== item);
		}
	}

	function handleSelectAll() {
		selectedItem = {};
		selectedItems = [];
		if (selectAll) {
			selectedItems = items;
		}
	}

	function handleItemSelect(item: any) {
		onselect(item);
		selectedItem = item;
	}

	function handleKeyDown(
		event: KeyboardEvent & { currentTarget: EventTarget & HTMLButtonElement },
		item: any
	): any {
		if (event.key === 'Enter' || event.key === ' ') {
			handleItemSelect(item);
		}
	}

	function isSelected(item: any) {
		return item === selectedItem || selectedItems.includes(item);
	}
</script>

<div class={baseClass}>
	<ul class={listClass}>
		<li class="bg-surface-900 sticky top-0 flex list-item cursor-pointer items-center p-2">
			<div class={headerClass}>
				<Checkbox bind:checked={selectAll} onchange={handleSelectAll} />
				{@render listHeader()}
			</div>
		</li>
		{#each items as item}
			<li class={cn(itemClass, isSelected(item) ? 'bg-secondary-500/25' : '')}>
				<Checkbox
					checked={selectedItems.includes(item)}
					onchange={(isChecked) => handleCheckboxChange(item, isChecked)}
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
							onclick={() => handleItemSelect(item)}
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
