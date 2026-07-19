<script lang="ts" generics="T extends Record<string, any> | string">
	import { cn } from '$theme';
	import type { Snippet } from 'svelte';
	import { Checkbox, Tooltip } from '$components/ui';
	import { GripVertical } from 'lucide-svelte';

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
		reorderable = false,
		onReorder,
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
		reorderable?: boolean;
		onReorder?: (orderedIds: (string | number)[]) => void;
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
		if (Array.isArray(item) && item.length === 2) {
			return item[0];
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

	let dragSourceIndex: number | null = $state(null);
	let dropIndex: number | null = $state(null);
	let dragGhost: HTMLElement | null = null;

	function handleDragStart(event: DragEvent, index: number) {
		dragSourceIndex = index;
		if (!event.dataTransfer) return;
		// Required for a native drag to actually start (Firefox) and to show
		// the move cursor instead of the no-drop cursor.
		event.dataTransfer.effectAllowed = 'move';
		event.dataTransfer.setData('text/plain', String(index));

		// Drag the whole row as an opaque card instead of just the grip handle.
		// Rows are transparent, so clone into an off-screen styled chip and use
		// that as the drag image, anchored under the cursor.
		const row = (event.currentTarget as HTMLElement).closest('li');
		if (row) {
			const rect = row.getBoundingClientRect();
			const ghost = row.cloneNode(true) as HTMLElement;
			ghost.classList.add('bg-surface-800', 'rounded-sm', 'shadow-lg');
			ghost.style.position = 'fixed';
			ghost.style.top = '-1000px';
			ghost.style.left = '0';
			ghost.style.width = `${rect.width}px`;
			ghost.style.pointerEvents = 'none';
			document.body.appendChild(ghost);
			dragGhost = ghost;
			event.dataTransfer.setDragImage(ghost, event.clientX - rect.left, event.clientY - rect.top);
		}
	}

	function handleDragEnd() {
		dragSourceIndex = null;
		dropIndex = null;
		dragGhost?.remove();
		dragGhost = null;
	}

	// The gap the item will drop into: `dropIndex` items from the top, so gap i
	// is the line above row i and gap items.length is below the last row.
	function handleDragOver(event: DragEvent, index: number) {
		if (!reorderable || dragSourceIndex === null) return;
		event.preventDefault();
		if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
		const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
		const inBottomHalf = event.clientY - rect.top > rect.height / 2;
		dropIndex = inBottomHalf ? index + 1 : index;
	}

	function handleDrop() {
		if (!reorderable || dragSourceIndex === null || dropIndex === null) {
			handleDragEnd();
			return;
		}
		const reordered = [...items];
		const [moved] = reordered.splice(dragSourceIndex, 1);
		// The removed source shifts every gap after it left by one.
		const insertAt = dropIndex > dragSourceIndex ? dropIndex - 1 : dropIndex;
		reordered.splice(insertAt, 0, moved);
		handleDragEnd();
		onReorder?.(reordered.map(getItemKey));
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
		{#each items as item, i (getItemKey(item))}
			<li
				class={cn(
					itemClass,
					isSelected(item) ? 'bg-secondary-500/25' : '',
					reorderable ? 'relative' : ''
				)}
				ondragover={(e) => handleDragOver(e, i)}
				ondrop={handleDrop}
			>
				{#if reorderable && dragSourceIndex !== null && dropIndex === i}
					<div class="bg-primary-500 pointer-events-none absolute inset-x-0 top-0 z-10 h-1"></div>
				{/if}
				{#if reorderable && dragSourceIndex !== null && dropIndex === i + 1 && i === items.length - 1}
					<div
						class="bg-primary-500 pointer-events-none absolute inset-x-0 bottom-0 z-10 h-1"
					></div>
				{/if}
				{#if reorderable}
					<span
						class="text-surface-400 mr-1 cursor-grab active:cursor-grabbing"
						draggable="true"
						ondragstart={(e) => handleDragStart(e, i)}
						ondragend={handleDragEnd}
						role="button"
						tabindex="0"
						aria-label="Drag to reorder"
					>
						<GripVertical class="h-4 w-4" />
					</span>
				{/if}
				{#if canSelect}
					<Checkbox
						checked={selectedItemsSet.has(getItemKey(item))}
						onchange={() => handleCheckboxChange(item)}
						class="mr-2"
					/>
				{/if}
				{#if listItemPopup}
					<Tooltip
						background="bg-surface-900"
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
				{:else}
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
				{/if}
			</li>
		{/each}
	</ul>
</div>
