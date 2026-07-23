<script lang="ts">
	import type { MapFeatureType } from './features';
	import OriginHover from './OriginHover.svelte';
	import PlayerHover from './PlayerHover.svelte';
	import BaseHover from './BaseHover.svelte';
	import FastTravelHover from './FastTravelHover.svelte';
	import RelicHover from './RelicHover.svelte';
	import DungeonHover from './DungeonHover.svelte';
	import BossHover from './BossHover.svelte';
	import PalHover from './PalHover.svelte';

	let { type, data, guildName }: { type: MapFeatureType; data: any; guildName?: string } = $props();
</script>

{#if type === 'origin'}
	<OriginHover />
{:else if type === 'player'}
	<PlayerHover player={data} />
{:else if type === 'base'}
	<BaseHover base={data} {guildName} />
{:else if type === 'fast_travel'}
	<FastTravelHover point={data} />
{:else if type === 'relic'}
	<RelicHover point={data} />
{:else if type === 'dungeon'}
	<DungeonHover point={data} />
{:else if type === 'boss'}
	<BossHover point={data} />
{:else if type === 'alpha_pal'}
	<PalHover point={data} isPredator={false} />
{:else if type === 'predator_pal'}
	<PalHover point={data} isPredator={true} />
{/if}
