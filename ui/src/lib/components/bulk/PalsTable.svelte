<script lang="ts">
	import { Table, Input, Loading } from '$components/ui';
	import type { ColumnDef } from '$components/ui/table/table.types';
	import { PawPrint } from '@lucide/svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { sendAndWait } from '$lib/utils/websocketUtils';
	import { MessageType, type PalSummary } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { palsData, elementsData } from '$lib/data';
	import { assetLoader } from '$utils';
	import { filterBySearch } from './bulk.utils';

	let { selected = $bindable(new Set<string>()) }: { selected?: Set<string> } = $props();

	type PalRow = PalSummary & { species_name: string };

	let summaries: PalSummary[] = $state([]);
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
		</div>
		{#if loadingRows && summaries.length === 0}
			<div class="relative flex flex-1 items-center justify-center">
				<Loading
					label={m.loading_entity({ entity: c.pals })}
					loadingComplete={!loadingRows}
					icon={PawPrint}
				/>
			</div>
		{:else}
			<Table {rows} {columns} rowKey={(row) => row.instance_id} pageSize={15} bind:selected>
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
				{#snippet empty()}
					{m.no_pals_match()}
				{/snippet}
			</Table>
		{/if}
	</div>
</div>
