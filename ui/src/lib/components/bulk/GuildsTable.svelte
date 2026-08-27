<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Table, Input, Button, Checkbox, Tooltip } from '$components/ui';
	import type { ColumnDef } from '$components/ui/table/table.types';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { getAppState, getModalState, getToastState } from '$states';
	import { send } from '$lib/utils/websocketUtils';
	import { MessageType } from '$types';
	import { cn } from '$theme';
	import { buildGuildRows, filterBySearch, emptyGuildIds, type GuildRow } from './bulk.utils';
	import BulkSelectionBanner from './BulkSelectionBanner.svelte';
	import GuildDetailPanel from './GuildDetailPanel.svelte';

	let { selected = $bindable(new Set<string>()) }: { selected?: Set<string> } = $props();

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();
	let query = $state('');
	let detailOpen = $state(false);
	let viewMode = $state<'grid' | 'list'>('grid');

	function openDetail(row: GuildRow) {
		detailOpen = true;
		appState.bulkDetailGuild = undefined;
		appState.loadGuildDetailsForBulk(row.id);
	}

	function closeDetail() {
		detailOpen = false;
		appState.bulkDetailGuild = undefined;
	}

	const playerNameByUid = $derived(
		new Map(appState.playerSummariesArray.map((player) => [player.uid, player.nickname]))
	);
	const allRows = $derived(buildGuildRows(appState.guildSummariesArray, playerNameByUid));
	const rows = $derived(filterBySearch(allRows, query, ['name', 'id', 'leaderName']));

	const columns: ColumnDef<GuildRow>[] = [
		{ key: 'name', header: 'Guild Name', sortable: true },
		{ key: 'id', header: 'Guild ID', sortable: true },
		{ key: 'player_count', header: 'Members', sortable: true, align: 'right' },
		{ key: 'pal_count', header: 'Pals', sortable: true, align: 'right' },
		{ key: 'level', header: 'Level', sortable: true, align: 'right' },
		{ key: 'base_count', header: 'Bases', sortable: true, align: 'right' },
		{ key: 'leaderName', header: 'Leader', sortable: true }
	];

	function toggleSelected(id: string) {
		const next = new Set(selected);
		if (next.has(id)) {
			next.delete(id);
		} else {
			next.add(id);
		}
		selected = next;
	}

	function deleteIds(ids: string[]) {
		for (const id of ids) {
			send(MessageType.DELETE_GUILD, { guild_id: id, origin: 'bulk' });
		}
		toast.add(m.deleted_entity({ entity: c.guilds, count: ids.length }), m.success(), 'success');
		selected = new Set<string>();
	}

	async function deleteOne(row: GuildRow) {
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

	function leaderLabel(row: GuildRow): string {
		return row.leaderName ?? row.leaderUid ?? '—';
	}
</script>

<div class="flex h-full min-h-0">
	<div class="mr-2 flex min-w-0 flex-1 flex-col gap-2 overflow-y-auto">
		<div class="flex items-center gap-2">
			<Input bind:value={query} placeholder={m.bulk_search_placeholder({ entity: c.guilds })} />
			<div class="bg-surface-900 flex items-center gap-2 rounded-sm p-1">
				<Tooltip
					label={m.delete_selected_entity({ entity: c.guilds })}
					disabled={selected.size === 0}
				>
					<Button variant="ghost" disabled={selected.size === 0} onclick={bulkDelete}>
						<Icon icon="tabler:trash" class="h-4 w-4" />
					</Button>
				</Tooltip>

				<Tooltip label={m.delete_empty_guilds()}>
					<Button variant="ghost" onclick={deleteEmpty}>
						<Icon icon="tabler:trash-x" class="h-4 w-4" />
					</Button>
				</Tooltip>
			</div>
			<div class="bg-surface-900 flex items-center gap-1 rounded-sm p-1">
				<Tooltip label={m.grid_view()}>
					<button
						type="button"
						class={cn(
							'rounded-sm p-1.5 transition-colors',
							viewMode === 'grid'
								? 'bg-surface-700 text-primary-300'
								: 'text-surface-400 hover:bg-surface-800'
						)}
						onclick={() => (viewMode = 'grid')}
						aria-pressed={viewMode === 'grid'}
					>
						<Icon icon="tabler:layout-grid" class="h-4 w-4" />
					</button>
				</Tooltip>
				<Tooltip label={m.list_view()}>
					<button
						type="button"
						class={cn(
							'rounded-sm p-1.5 transition-colors',
							viewMode === 'list'
								? 'bg-surface-700 text-primary-300'
								: 'text-surface-400 hover:bg-surface-800'
						)}
						onclick={() => (viewMode = 'list')}
						aria-pressed={viewMode === 'list'}
					>
						<Icon icon="tabler:list" class="h-4 w-4" />
					</button>
				</Tooltip>
			</div>
		</div>
		<BulkSelectionBanner
			selectedCount={selected.size}
			matchingCount={rows.length}
			onSelectAll={selectAllMatching}
			onClear={clearSelection}
		/>
		{#if viewMode === 'grid'}
			{#if rows.length === 0}
				<p class="text-surface-400 py-8 text-center text-sm">{m.no_guilds_match()}</p>
			{:else}
				<div class="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
					{#each rows as row (row.id)}
						<div
							class={cn(
								'card card-hover bg-surface-800/60 relative cursor-pointer p-4',
								selected.has(row.id) && 'ring-primary-500 ring-1'
							)}
							role="button"
							tabindex="0"
							onclick={() => openDetail(row)}
							onkeydown={(event: KeyboardEvent) => {
								if (event.key === 'Enter' || event.key === ' ') {
									event.preventDefault();
									openDetail(row);
								}
							}}
						>
							<div class="flex items-start justify-between gap-2">
								<div class="flex min-w-0 items-center gap-2">
									<Icon
										icon="tabler:building-community"
										class="text-primary-500 h-5 w-5 shrink-0"
									/>
									<span class="truncate font-semibold">{row.name}</span>
								</div>
								<div
									role="presentation"
									onclick={(event) => event.stopPropagation()}
									onkeydown={(event) => event.stopPropagation()}
								>
									<Tooltip label={m.select_entity({ entity: row.name })}>
										<Checkbox
											checked={selected.has(row.id)}
											onchange={() => toggleSelected(row.id)}
										/>
									</Tooltip>
								</div>
							</div>
							<div class="mt-3 grid grid-cols-2 gap-x-4 gap-y-1.5">
								<span class="text-surface-400 flex items-center gap-1.5 text-xs">
									<Icon icon="tabler:users" class="h-3.5 w-3.5" />
									{row.player_count}
									{c.players}
								</span>
								<span class="text-surface-400 flex items-center gap-1.5 text-xs">
									<Icon icon="tabler:map-pin" class="h-3.5 w-3.5" />
									{row.base_count}
									{c.bases}
								</span>
								<span class="text-surface-400 flex items-center gap-1.5 text-xs">
									<Icon icon="tabler:star" class="h-3.5 w-3.5" />
									Lvl {row.level ?? '—'}
								</span>
								<span class="text-surface-400 flex items-center gap-1.5 text-xs">
									<Icon icon="tabler:paw" class="h-3.5 w-3.5" />
									{row.pal_count}
									{c.pals}
								</span>
							</div>
							<div class="border-surface-700 mt-3 flex items-center gap-1.5 border-t pt-2">
								<Icon icon="tabler:crown" class="text-warning-500 h-3.5 w-3.5 shrink-0" />
								<span class="text-surface-400 truncate text-xs">{leaderLabel(row)}</span>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		{:else}
			<Table
				{rows}
				{columns}
				rowKey={(row) => row.id}
				pageSize={15}
				bind:selected
				onrowclick={openDetail}
			>
				{#snippet cell({ row, column })}
					{#if column.key === 'level'}
						{row.level ?? '—'}
					{:else if column.key === 'leaderName'}
						<span class="flex items-center gap-1">
							{#if row.leaderUid}
								<Icon icon="tabler:crown" class="text-warning-500 h-3.5 w-3.5 shrink-0" />
							{/if}
							<span class="truncate">{leaderLabel(row)}</span>
						</span>
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
						<Icon icon="tabler:trash-x" class="h-4 w-4" />
					</Button>
				{/snippet}
				{#snippet empty()}
					{m.no_guilds_match()}
				{/snippet}
			</Table>
		{/if}
	</div>
	<GuildDetailPanel expanded={detailOpen} onclose={closeDetail} />
</div>
