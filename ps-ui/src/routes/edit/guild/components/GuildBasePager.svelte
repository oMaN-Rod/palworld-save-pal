<script lang="ts">
	import { Button, TooltipButton } from '$components/ui';
	import { staticIcons } from '$types/icons';

	interface Props {
		total: number;
		current: number;
		visibleCount?: number;
		onSelect: (base: number) => void;
		onPrevious: () => void;
		onNext: () => void;
	}

	let { total, current, visibleCount = 16, onSelect, onPrevious, onNext }: Props = $props();

	const windowStart = $derived(
		Math.max(1, Math.min(current - Math.floor(visibleCount / 2), total - visibleCount + 1))
	);

	const windowEnd = $derived(Math.min(windowStart + visibleCount - 1, total));

	const visibleBases = $derived(
		Array.from({ length: windowEnd - windowStart + 1 }, (_, i) => windowStart + i)
	);
</script>

<div id="guild-pager" class="mb-4 flex items-center justify-center space-x-4">
	<Button class="rounded-full p-0! font-bold" variant="ghost" size="md" onclick={onPrevious}>
		<img src={staticIcons.qIcon} alt="Previous" class="h-10 w-10" />
	</Button>

	<div class="flex space-x-2">
		{#each visibleBases as base}
			<TooltipButton
				buttonClass="h-8 w-8 rounded-full {base === current
					? 'bg-primary-500! text-white'
					: 'bg-surface-800 hover:bg-surface-600'}"
				onclick={() => onSelect(base)}
				popupLabel={`Box ${base}`}
				variant="ghost"
				size="md"
			>
				{Math.floor(base)}
			</TooltipButton>
		{/each}
	</div>

	<Button class="rounded-sm p-0! font-bold" variant="ghost" size="md" onclick={onNext}>
		<img src={staticIcons.eIcon} alt="Next" class="h-10 w-10" />
	</Button>
</div>
