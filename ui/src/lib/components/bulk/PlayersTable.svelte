<script lang="ts">
	import { Table, Input } from '$components/ui';
	import type { ColumnDef } from '$components/ui/table/table.types';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { getAppState } from '$states';
	import { buildPlayerRows, filterBySearch, daysSince, type PlayerRow } from './bulk.utils';

	let { selected = $bindable(new Set<string>()) }: { selected?: Set<string> } = $props();

	const appState = getAppState();
	let query = $state('');

	const allRows = $derived(
		buildPlayerRows(appState.playerSummariesArray, appState.guildSummariesArray)
	);
	const rows = $derived(filterBySearch(allRows, query, ['nickname', 'uid', 'guildName']));

	const columns: ColumnDef<PlayerRow>[] = [
		{ key: 'nickname', header: 'Player Name', sortable: true },
		{ key: 'uid', header: 'Player ID', sortable: true },
		{ key: 'level', header: 'Level', sortable: true, align: 'right' },
		{ key: 'guildName', header: 'Guild', sortable: true },
		{ key: 'pal_count', header: 'Pals', sortable: true, align: 'right' },
		{ key: 'lastOnline', header: 'Last Active', sortable: true }
	];

	function lastActiveLabel(row: PlayerRow): string {
		if (!row.lastOnline) return m.never_online();
		const dayCount = daysSince(row.lastOnline, Date.now());
		return dayCount === null ? m.never_online() : `${dayCount}d ago`;
	}
</script>

<div class="flex flex-col gap-2">
	<Input bind:value={query} placeholder={m.bulk_search_placeholder({ entity: c.players })} />
	<Table {rows} {columns} rowKey={(row) => row.uid} bind:selected>
		{#snippet cell({ row, column })}
			{#if column.key === 'lastOnline'}
				{lastActiveLabel(row)}
			{:else if column.key === 'level'}
				{row.level ?? '—'}
			{:else}
				{row[column.key as keyof PlayerRow]}
			{/if}
		{/snippet}
		{#snippet empty()}
			{m.no_players_match()}
		{/snippet}
	</Table>
</div>
