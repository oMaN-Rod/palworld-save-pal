<script lang="ts">
	import type { ActionDescriptor } from '$components/ui/actions';
	import type {
		PalContainerSelection,
		PalContainerServerPaging
	} from '$states/palContainer.svelte';

	import PalContainerView from '../PalContainerView.svelte';

	type HarnessPal = { id: number; nickname: string; element: string };

	let {
		pals,
		storageKey,
		pageSize = 4,
		actions = [],
		palActions,
		onOpenPal,
		serverPaging,
		selection,
		withFilters = true
	}: {
		pals: HarnessPal[];
		storageKey: string;
		pageSize?: number;
		actions?: ActionDescriptor[];
		palActions?: (pal: HarnessPal) => ActionDescriptor[];
		onOpenPal?: (pal: HarnessPal) => void;
		serverPaging?: PalContainerServerPaging;
		selection?: PalContainerSelection<number>;
		/** Off for the case where nothing at all is filterable. */
		withFilters?: boolean;
	} = $props();

	function matches(pal: HarnessPal, { query, filter }: { query: string; filter: string }): boolean {
		const byQuery = query === '' || pal.nickname.toLowerCase().includes(query.toLowerCase());
		const byFilter = filter === 'All' || pal.element === filter;
		return byQuery && byFilter;
	}
</script>

<PalContainerView
	{pals}
	idOf={(pal) => pal.id}
	nicknameOf={(pal) => pal.nickname}
	{storageKey}
	title="Palbox"
	{pageSize}
	{actions}
	matches={serverPaging ? undefined : matches}
	{serverPaging}
	{selection}
	filters={withFilters ? harnessFilters : undefined}
	{palActions}
	{onOpenPal}
>
	{#snippet portrait(pal)}
		<span data-testid="portrait-{pal.id}">{pal.nickname}</span>
	{/snippet}

	{#snippet columns(pal)}
		<span data-testid="columns-{pal.id}">HP {pal.id}00</span>
	{/snippet}

	{#snippet detail(pal)}
		<p data-testid="detail-body">Details for {pal.nickname}</p>
	{/snippet}
</PalContainerView>

{#snippet harnessFilters({
	selected,
	select
}: {
	selected: string;
	select: (value: string) => void;
})}
	<div data-testid="harness-filters">
		<button
			type="button"
			aria-label="Fire"
			aria-pressed={selected === 'fire'}
			onclick={() => select('fire')}>Fire</button
		>
		<button
			type="button"
			aria-label="All types"
			aria-pressed={selected === 'All'}
			onclick={() => select('All')}>All</button
		>
	</div>
{/snippet}
