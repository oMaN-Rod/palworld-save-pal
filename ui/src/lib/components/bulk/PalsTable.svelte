<script lang="ts">
	import { Table, Input, Loading, Button, Tooltip } from '$components/ui';
	import type { ColumnDef } from '$components/ui/table/table.types';
	import { PawPrint, Trash2, Trash } from '@lucide/svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { getAppState, getModalState, getToastState, getPalEditorState } from '$states';
	import { send, sendAndWait } from '$lib/utils/websocketUtils';
	import { MessageType, type PalSummary } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { palsData, elementsData } from '$lib/data';
	import { assetLoader } from '$utils';
	import { filterBySearch, resolveBulkPal } from './bulk.utils';
	import BulkSelectionBanner from './BulkSelectionBanner.svelte';

	let { selected = $bindable(new Set<string>()) }: { selected?: Set<string> } = $props();

	type PalRow = PalSummary & { species_name: string };

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();
	const palEditor = getPalEditorState();
	let pendingPalId = $state<string | null>(null);
	let openedFromTable = $state(false);

	function openEditor(row: PalRow) {
		pendingPalId = row.instance_id;
		openedFromTable = true;
		palEditor.openLoading();
		if (row.owner_uid) {
			appState.bulkDetailPlayer = undefined;
			appState.loadPlayerDetailsForBulk(row.owner_uid);
		} else if (row.guild_id) {
			appState.bulkDetailGuild = undefined;
			appState.loadGuildDetailsForBulk(row.guild_id);
		}
	}

	const resolvedPal = $derived(
		resolveBulkPal(appState.bulkDetailPlayer, appState.bulkDetailGuild, pendingPalId)
	);

	$effect(() => {
		if (pendingPalId && resolvedPal) {
			palEditor.resolve(resolvedPal);
			pendingPalId = null;
		}
	});

	$effect(() => {
		if (!palEditor.isOpen && openedFromTable) {
			openedFromTable = false;
			loadSummaries();
		}
	});

	// $state.raw: rows are read-only display data, reassigned wholesale on refetch.
	// Deep-proxying 1700+ rows makes the allRows map take seconds in dev.
	let summaries: PalSummary[] = $state.raw([]);
	let loadingRows = $state(false);
	let query = $state('');

	async function loadSummaries() {
		loadingRows = true;
		try {
			const response = await sendAndWait<{ pals: PalSummary[] }>(MessageType.GET_PAL_SUMMARIES);
			summaries = response.pals ?? [];
		} finally {
			loadingRows = false;
		}
	}

	$effect(() => {
		loadSummaries();
	});

	const allRows: PalRow[] = $derived(
		summaries.map((pal) => ({
			...pal,
			species_name: palsData.getByKey(pal.character_key)?.localized_name || pal.character_id
		}))
	);
	const rows = $derived(
		filterBySearch(allRows, query, ['nickname', 'species_name', 'character_id', 'owner_name'])
	);

	function groupPalIds(ids: string[]) {
		const rowById = new Map(allRows.map((row) => [row.instance_id, row]));
		const byOwner = new Map<string, string[]>();
		const byBase = new Map<string, { guildId: string; baseId: string; palIds: string[] }>();
		for (const id of ids) {
			const row = rowById.get(id);
			if (!row) continue;
			if (row.owner_uid) {
				const group = byOwner.get(row.owner_uid) ?? [];
				group.push(id);
				byOwner.set(row.owner_uid, group);
			} else if (row.guild_id && row.base_id) {
				const key = `${row.guild_id}:${row.base_id}`;
				const group = byBase.get(key) ?? { guildId: row.guild_id, baseId: row.base_id, palIds: [] };
				group.palIds.push(id);
				byBase.set(key, group);
			}
		}
		return { byOwner, byBase };
	}

	async function deletePalIds(ids: string[]) {
		const { byOwner, byBase } = groupPalIds(ids);
		for (const [ownerUid, palIds] of byOwner) {
			send(MessageType.DELETE_PALS, { player_id: ownerUid, pal_ids: palIds });
		}
		for (const group of byBase.values()) {
			send(MessageType.DELETE_PALS, {
				guild_id: group.guildId,
				base_id: group.baseId,
				pal_ids: group.palIds
			});
		}
		toast.add(m.deleted_entity({ entity: c.pals, count: ids.length }), m.success(), 'success');
		selected = new Set<string>();
		await loadSummaries();
	}

	async function deleteOne(row: PalRow) {
		// @ts-ignore
		const confirmed = await modal.showConfirmModal({
			title: m.delete_entity({ entity: c.pal }),
			message: m.delete_entity_by_name_confirm({ name: row.nickname || row.species_name }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});
		if (confirmed) await deletePalIds([row.instance_id]);
	}

	async function bulkDelete() {
		const ids = [...selected];
		if (ids.length === 0) return;
		// @ts-ignore
		const confirmed = await modal.showConfirmModal({
			title: m.delete_selected_entity({ entity: c.pals }),
			message: m.delete_count_entities_confirm({ count: ids.length, entity: c.pals }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});
		if (confirmed) await deletePalIds(ids);
	}

	const columns: ColumnDef<PalRow>[] = [
		{ key: 'nickname', header: 'Nickname', sortable: true },
		{ key: 'species_name', header: 'Species', sortable: true },
		{ key: 'character_id', header: 'Species ID', sortable: true },
		{ key: 'elements', header: 'Element', sortable: false },
		{ key: 'owner_name', header: 'Owner', sortable: true },
		{ key: 'gender', header: 'Gender', sortable: true },
		{ key: 'level', header: 'Level', sortable: true, align: 'right' },
		{ key: 'hp', header: 'HP', sortable: true, align: 'right', sortValue: (row) => row.hp },
		{ key: 'stomach', header: 'Stomach', sortable: true, align: 'right' },
		{ key: 'rank', header: 'Rank', sortable: true, align: 'right' },
		{ key: 'exp', header: 'Exp', sortable: true, align: 'right' },
		{ key: 'talents', header: 'Talents', sortable: false },
		{ key: 'souls', header: 'Souls', sortable: false }
	];

	function elementIcon(elementType: string): string {
		const elementInfo = elementsData.elements[elementType];
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${elementInfo?.icon}.webp`);
	}

	function genderIcon(gender: string): string {
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${gender}.webp`);
	}
</script>

<div class="flex h-full min-h-0">
	<div class="mr-2 flex min-w-0 flex-1 flex-col gap-2 overflow-y-auto">
		<div class="flex items-center gap-2">
			<Input bind:value={query} placeholder={m.bulk_search_placeholder({ entity: c.pals })} />
			<div class="bg-surface-900 flex items-center gap-2 rounded-sm p-1">
				<Tooltip
					label={m.delete_selected_entity({ entity: c.pals })}
					disabled={selected.size === 0}
				>
					<Button variant="ghost" disabled={selected.size === 0} onclick={bulkDelete}>
						<Trash class="h-4 w-4" />
					</Button>
				</Tooltip>
			</div>
		</div>
		<BulkSelectionBanner
			selectedCount={selected.size}
			matchingCount={rows.length}
			onSelectAll={() => (selected = new Set(rows.map((row) => row.instance_id)))}
			onClear={() => (selected = new Set<string>())}
		/>
		{#if loadingRows && summaries.length === 0}
			<div class="relative flex flex-1 items-center justify-center">
				<Loading
					label={m.loading_entity({ entity: c.pals })}
					loadingComplete={!loadingRows}
					icon={PawPrint}
				/>
			</div>
		{:else}
			<Table
				{rows}
				{columns}
				rowKey={(row) => row.instance_id}
				pageSize={15}
				bind:selected
				onrowclick={openEditor}
			>
				{#snippet cell({ row, column })}
					{#if column.key === 'nickname'}
						{row.nickname ?? '—'}
					{:else if column.key === 'elements'}
						<div class="flex gap-1">
							{#each palsData.getByKey(row.character_key)?.element_types ?? [] as elementType}
								<img src={elementIcon(elementType)} alt={elementType} class="h-4 w-4" />
							{/each}
						</div>
					{:else if column.key === 'gender'}
						{#if row.gender}
							<img src={genderIcon(row.gender)} alt={row.gender} class="h-4 w-4" />
						{:else}
							—
						{/if}
					{:else if column.key === 'hp'}
						{Math.round(row.hp / 1000)}
					{:else if column.key === 'talents'}
						{`${row.talent_hp}/${row.talent_shot}/${row.talent_defense}`}
					{:else if column.key === 'souls'}
						{`${row.rank_hp}/${row.rank_attack}/${row.rank_defense}/${row.rank_craftspeed}`}
					{:else}
						{row[column.key as keyof PalRow]}
					{/if}
				{/snippet}
				{#snippet rowActions(row)}
					<Button
						variant="ghost"
						onclick={() => deleteOne(row)}
						title={m.delete_entity({ entity: c.pal })}
					>
						<Trash2 class="h-4 w-4" />
					</Button>
				{/snippet}
				{#snippet empty()}
					{m.no_pals_match()}
				{/snippet}
			</Table>
		{/if}
	</div>
</div>
