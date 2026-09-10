<script lang="ts">
	import { Button, TooltipButton } from '$components/ui';
	import { staticIcons } from '$types/icons';
	import * as m from '$i18n/messages';

	const VISIBLE_BUBBLES = 5;

	let {
		page = 0,
		pageCount = 0,
		disabled = false,
		id = 'live-palbox-pager',
		labelFor = (index: number) => m.box_number({ number: index + 1 }),
		onPageChange
	}: {
		page?: number;
		pageCount?: number;
		disabled?: boolean;
		id?: string;
		labelFor?: (index: number) => string;
		onPageChange: (page: number) => void;
	} = $props();

	const firstVisible = $derived(
		Math.max(
			0,
			Math.min(page - Math.floor(VISIBLE_BUBBLES / 2), pageCount - VISIBLE_BUBBLES)
		)
	);
	const visiblePages = $derived(
		Array.from({ length: Math.min(VISIBLE_BUBBLES, pageCount) }, (_, i) => firstVisible + i)
	);

	function go(next: number) {
		if (disabled || pageCount <= 0) return;
		const wrapped = ((next % pageCount) + pageCount) % pageCount;
		if (wrapped !== page) onPageChange(wrapped);
	}
</script>

{#if pageCount > 1}
	<div {id} class="flex items-center justify-center space-x-4">
		<Button
			class="rounded-full p-0! font-bold"
			variant="ghost"
			size="md"
			{disabled}
			title={m.previous()}
			onclick={() => go(page - 1)}
		>
			<img src={staticIcons.qIcon} alt={m.previous()} class="h-10 w-10" />
		</Button>

		<div class="flex space-x-2">
			{#each visiblePages as boxPage (boxPage)}
				<TooltipButton
					buttonClass="h-8 w-8 rounded-full {boxPage === page
						? 'bg-primary-500! text-white'
						: 'bg-surface-800 hover:bg-surface-600'}"
					popupLabel={labelFor(boxPage)}
					variant="ghost"
					size="md"
					{disabled}
					onclick={() => go(boxPage)}
				>
					{boxPage + 1}
				</TooltipButton>
			{/each}
		</div>

		<Button
			class="rounded-full p-0! font-bold"
			variant="ghost"
			size="md"
			{disabled}
			title={m.next()}
			onclick={() => go(page + 1)}
		>
			<img src={staticIcons.eIcon} alt={m.next()} class="h-10 w-10" />
		</Button>
	</div>
{/if}
