<script lang="ts">
	import { Table, Input, Button, Popover, Tooltip } from '$components/ui';
	import type { ColumnDef } from '$components/ui/table/table.types';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { getAppState, getModalState, getNavigationState, getToastState } from '$states';
	import {
		buildPlayerRows,
		filterBySearch,
		daysSince,
		inactivePlayerUids,
		type PlayerRow
	} from './bulk.utils';
	import { Pencil, Trash2 } from 'lucide-svelte';
	import { send } from '$lib/utils/websocketUtils';
	import { MessageType } from '$types';
	import BulkSelectionBanner from './BulkSelectionBanner.svelte';
	import { ClockAlert, Trash } from '@lucide/svelte';

	let { selected = $bindable(new Set<string>()) }: { selected?: Set<string> } = $props();

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();
	const nav = getNavigationState();
	let query = $state('');
	let inactiveDays = $state(30);

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

	function editPlayer(uid: string) {
		if (appState.players[uid]) {
			appState.selectedPlayer = appState.players[uid];
			nav.saveAndNavigate('/edit/player');
		} else {
			appState.selectPlayerLazy(uid);
		}
	}

	function deleteUids(uids: string[]) {
		for (const uid of uids) {
			send(MessageType.DELETE_PLAYER, { player_id: uid, origin: 'bulk' });
		}
		selected = new Set<string>();
	}

	async function deleteOne(row: PlayerRow) {
		// @ts-ignore
		const confirmed = await modal.showConfirmModal({
			title: m.delete_entity({ entity: c.player }),
			message: m.delete_entity_by_name_confirm({ name: row.nickname }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});
		if (confirmed) deleteUids([row.uid]);
	}

	async function bulkDelete() {
		const uids = [...selected];
		if (uids.length === 0) return;
		// @ts-ignore
		const confirmed = await modal.showConfirmModal({
			title: m.delete_selected_entity({ entity: c.players }),
			message: m.delete_count_entities_confirm({ count: uids.length, entity: c.players }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});
		if (confirmed) deleteUids(uids);
	}

	async function deleteInactive() {
		const uids = inactivePlayerUids(rows, inactiveDays, Date.now());
		if (uids.length === 0) {
			toast.add(m.no_players_match(), undefined, 'info');
			return;
		}
		// @ts-ignore
		const confirmed = await modal.showConfirmModal({
			title: m.delete_inactive_players(),
			message: m.delete_count_entities_confirm({ count: uids.length, entity: c.players }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});
		if (confirmed) deleteUids(uids);
	}

	function selectAllMatching() {
		selected = new Set(rows.map((row) => row.uid));
	}

	function clearSelection() {
		selected = new Set<string>();
	}
</script>

<div class="flex flex-col gap-2">
	<Input bind:value={query} placeholder={m.bulk_search_placeholder({ entity: c.players })} />
	<div class="bg-surface-900 flex items-center gap-2 rounded-sm p-1">
		<Popover position="bottom-end">
			<Tooltip label={m.delete_inactive_players()}>
				<Button variant="ghost">
					<ClockAlert class="h-4 w-4" />
				</Button>
			</Tooltip>
			{#snippet content({ close })}
				<div class="flex flex-col gap-3">
					<label class="flex flex-col gap-1">
						<span class="text-sm font-medium">{m.inactivity_days_label()}</span>
						<Input
							type="number"
							min={1}
							bind:value={inactiveDays}
							class="input bg-surface-900 w-40"
						/>
					</label>
					<Button
						variant="danger"
						type="submit"
						onclick={(e: Event) => {
							e.preventDefault();
							close();
							deleteInactive();
						}}
					>
						{m.delete_inactive_players()}
					</Button>
				</div>
			{/snippet}
		</Popover>
		{#if selected.size > 0}
			<Tooltip label={m.delete_selected_entity({ entity: c.players })}>
				<Button variant="ghost" class="hover:bg-error-500" onclick={bulkDelete}>
					<Trash class="h-4 w-4" />
				</Button>
			</Tooltip>
		{/if}
	</div>
	<BulkSelectionBanner
		selectedCount={selected.size}
		matchingCount={rows.length}
		onSelectAll={selectAllMatching}
		onClear={clearSelection}
	/>
	<Table {rows} {columns} rowKey={(row) => row.uid} pageSize={15} bind:selected>
		{#snippet cell({ row, column })}
			{#if column.key === 'lastOnline'}
				{lastActiveLabel(row)}
			{:else if column.key === 'level'}
				{row.level ?? '—'}
			{:else}
				{row[column.key as keyof PlayerRow]}
			{/if}
		{/snippet}
		{#snippet rowActions(row)}
			<div class="flex gap-1">
				<Button
					variant="ghost"
					onclick={() => editPlayer(row.uid)}
					title={m.edit_entity({ entity: c.player })}
				>
					<Pencil class="h-4 w-4" />
				</Button>
				<Button
					variant="ghost"
					onclick={() => deleteOne(row)}
					title={m.delete_entity({ entity: c.player })}
				>
					<Trash2 class="h-4 w-4" />
				</Button>
			</div>
		{/snippet}
		{#snippet empty()}
			{m.no_players_match()}
		{/snippet}
	</Table>
</div>
