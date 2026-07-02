<script lang="ts">
	import { Table, Input, Button } from '$components/ui';
	import type { ColumnDef } from '$components/ui/table/table.types';
	import { Trash2 } from 'lucide-svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { getAppState, getModalState, getToastState } from '$states';
	import { send } from '$lib/utils/websocketUtils';
	import { MessageType } from '$types';
	import { buildGuildRows, filterBySearch, emptyGuildIds, type GuildRow } from './bulk.utils';
	import BulkSelectionBanner from './BulkSelectionBanner.svelte';

	let { selected = $bindable(new Set<string>()) }: { selected?: Set<string> } = $props();

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();
	let query = $state('');

	const allRows = $derived(buildGuildRows(appState.guildSummariesArray));
	const rows = $derived(filterBySearch(allRows, query, ['name', 'id']));

	const columns: ColumnDef<GuildRow>[] = [
		{ key: 'name', header: 'Guild Name', sortable: true },
		{ key: 'id', header: 'Guild ID', sortable: true },
		{ key: 'player_count', header: 'Members', sortable: true, align: 'right' },
		{ key: 'pal_count', header: 'Pals', sortable: true, align: 'right' },
		{ key: 'level', header: 'Level', sortable: true, align: 'right' },
		{ key: 'base_count', header: 'Bases', sortable: true, align: 'right' }
	];

	function deleteIds(ids: string[]) {
		for (const id of ids) {
			send(MessageType.DELETE_GUILD, { guild_id: id, origin: 'bulk' });
		}
		selected = new Set<string>();
	}

	async function deleteOne(row: GuildRow) {
		// @ts-ignore
		const confirmed = await modal.showConfirmModal({
			title: m.delete_entity({ entity: c.guild }),
			message: m.delete_entity_by_name_confirm({ name: row.name }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});
		if (confirmed) deleteIds([row.id]);
	}

	async function bulkDelete() {
		const ids = [...selected];
		if (ids.length === 0) return;
		// @ts-ignore
		const confirmed = await modal.showConfirmModal({
			title: m.delete_selected_entity({ entity: c.guilds }),
			message: m.delete_count_entities_confirm({ count: ids.length, entity: c.guilds }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});
		if (confirmed) deleteIds(ids);
	}

	async function deleteEmpty() {
		const ids = emptyGuildIds(rows);
		if (ids.length === 0) {
			toast.add(m.no_guilds_match(), undefined, 'info');
			return;
		}
		// @ts-ignore
		const confirmed = await modal.showConfirmModal({
			title: m.delete_empty_guilds(),
			message: m.delete_count_entities_confirm({ count: ids.length, entity: c.guilds }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});
		if (confirmed) deleteIds(ids);
	}

	function selectAllMatching() {
		selected = new Set(rows.map((row) => row.id));
	}

	function clearSelection() {
		selected = new Set<string>();
	}
</script>

<div class="flex min-h-0 flex-1 flex-col gap-2">
	<div class="flex flex-wrap items-center gap-2">
		<Input bind:value={query} placeholder={m.bulk_search_placeholder({ entity: c.guilds })} />
		<Button variant="danger" disabled={selected.size === 0} onclick={bulkDelete}>
			{m.delete_selected_entity({ entity: c.guilds })}
		</Button>
		<Button variant="danger" onclick={deleteEmpty}>{m.delete_empty_guilds()}</Button>
	</div>
	<BulkSelectionBanner
		selectedCount={selected.size}
		matchingCount={rows.length}
		onSelectAll={selectAllMatching}
		onClear={clearSelection}
	/>
	<Table {rows} {columns} rowKey={(row) => row.id} pageSize={15} bind:selected>
		{#snippet cell({ row, column })}
			{#if column.key === 'level'}
				{row.level ?? '—'}
			{:else}
				{row[column.key as keyof GuildRow]}
			{/if}
		{/snippet}
		{#snippet rowActions(row)}
			<Button
				variant="ghost"
				onclick={() => deleteOne(row)}
				title={m.delete_entity({ entity: c.guild })}
			>
				<Trash2 class="h-4 w-4" />
			</Button>
		{/snippet}
		{#snippet empty()}
			{m.no_guilds_match()}
		{/snippet}
	</Table>
</div>
