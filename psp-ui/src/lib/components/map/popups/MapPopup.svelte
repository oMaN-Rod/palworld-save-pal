<script lang="ts">
	import type { MapFeatureType } from '../features/features';
	import {
		BasePopup,
		BossPopup,
		FastTravelPopup,
		featureTypeLabel,
		OriginPopup,
		PalPopup,
		PlayerPopup,
		Popup,
		RelicPopup,
		StructurePopup
	} from './';

	let {
		type,
		data,
		guildName,
		onExportBase,
		onDeleteBase
	}: {
		type: MapFeatureType;
		data: any;
		guildName?: string;
		onExportBase?: (base: any) => void;
		onDeleteBase?: (base: any) => void;
	} = $props();

	const coords = $derived(
		typeof data?.x === 'number' ? { x: data.x, y: data.y, z: data.z ?? 0 } : undefined
	);
</script>

{#if type === 'origin'}
	<OriginPopup />
{:else if type === 'player'}
	<PlayerPopup player={data} />
{:else if type === 'base'}
	<BasePopup base={data} {guildName} onExport={onExportBase} {onDeleteBase} />
{:else if type === 'fast_travel'}
	<FastTravelPopup point={data} />
{:else if type === 'relic'}
	<RelicPopup point={data} />
{:else if type === 'boss'}
	<BossPopup boss={data} />
{:else if type === 'alpha_pal' || type === 'predator_pal'}
	<PalPopup point={data} isPredator={type === 'predator_pal'} />
{:else if type === 'structure' && data}
	<StructurePopup structure={data} />
{:else}
	<Popup title={featureTypeLabel(type)} {coords} />
{/if}
