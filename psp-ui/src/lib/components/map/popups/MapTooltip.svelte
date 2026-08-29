<script lang="ts">
	import type { MapFeatureType } from '../features/features';
	import {
		BaseHover,
		BossHover,
		FastTravelHover,
		featureTypeLabel,
		Hover,
		OriginHover,
		PalHover,
		PlayerHover,
		RelicHover,
		StructureHover
	} from './';

	let { type, data, guildName }: { type: MapFeatureType; data: any; guildName?: string } = $props();

	const coords = $derived(
		typeof data?.x === 'number' ? { x: data.x, y: data.y, z: data.z ?? 0 } : undefined
	);
</script>

{#if type === 'origin'}
	<OriginHover />
{:else if type === 'player'}
	<PlayerHover point={data} />
{:else if type === 'base'}
	<BaseHover base={data} {guildName} />
{:else if type === 'fast_travel'}
	<FastTravelHover point={data} />
{:else if type === 'relic'}
	<RelicHover point={data} />
{:else if type === 'boss'}
	<BossHover boss={data} />
{:else if type === 'alpha_pal' || type === 'predator_pal'}
	<PalHover point={data} isPredator={type === 'predator_pal'} />
{:else if type === 'structure' && data}
	<StructureHover structure={data} />
{:else}
	<Hover title={featureTypeLabel(type)} {coords} />
{/if}
