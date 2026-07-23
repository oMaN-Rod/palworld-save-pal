<script lang="ts">
	import type { MapFeatureType } from './features';
	import OriginPopup from './OriginPopup.svelte';
	import PlayerPopup from './PlayerPopup.svelte';
	import BasePopup from './BasePopup.svelte';
	import FastTravelPopup from './FastTravelPopup.svelte';
	import RelicPopup from './RelicPopup.svelte';
	import DungeonPopup from './DungeonPopup.svelte';
	import BossPopup from './BossPopup.svelte';
	import PalPopup from './PalPopup.svelte';

	let { type, data, guildName }: { type: MapFeatureType; data: any; guildName?: string } = $props();
</script>

{#if type === 'origin'}
	<OriginPopup />
{:else if type === 'player'}
	<PlayerPopup player={data} />
{:else if type === 'base'}
	<BasePopup base={data} {guildName} />
{:else if type === 'fast_travel'}
	<FastTravelPopup point={data} />
{:else if type === 'relic'}
	<RelicPopup point={data} />
{:else if type === 'dungeon'}
	<DungeonPopup point={data} />
{:else if type === 'boss'}
	<BossPopup point={data} />
{:else if type === 'alpha_pal'}
	<PalPopup point={data} isPredator={false} />
{:else if type === 'predator_pal'}
	<PalPopup point={data} isPredator={true} />
{/if}
