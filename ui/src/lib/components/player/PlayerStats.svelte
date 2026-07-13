<script lang="ts">
	import { EntryState, type Player } from '$types';
	import { staticIcons } from '$types/icons';
	import { getModalState } from '$states';
	import { NumberSliderModal } from '$components/modals';
	import { CornerDotButton } from '$components/ui';
	import { relicData } from '$lib/data';
	import type { RelicRankData } from '$lib/data/relic.svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let { player = $bindable() } = $props<{
		player: Player;
	}>();

	const modal = getModalState();

	let health = $derived(500 + player.status_point_list.max_hp * 100);
	let stamina = $derived(100 + player.status_point_list.max_sp * 10);
	let attack = $derived(100 + player.status_point_list.attack * 2);
	let workSpeed = $derived(100 + player.status_point_list.work_speed * 50);
	let weight = $derived(300 + player.status_point_list.weight * 50);

	// The five stats with known display formulas, rendered above. Everything else in
	// status_point_list is a relic-backed rank. Iterating the keys the save actually
	// has matters: writes are mutate-only, so offering to edit a missing row would
	// silently no-op.
	const HERO_STATS = ['max_hp', 'max_sp', 'attack', 'work_speed', 'weight'];

	// The status stat is `capture_rate`; its relic type is `capture_power`.
	function relicKeyFor(stat: string): string {
		return stat === 'capture_rate' ? 'capture_power' : stat;
	}

	// Stays `{}` while the fetch is in flight, and if it fails. Either way we do not
	// know any rank's cap, and every rank editor stays disabled until we do -- see
	// `updateExtraStat`.
	let relics: Record<string, RelicRankData> = $state({});
	$effect(() => {
		relicData
			.getRelicData()
			.then((data) => (relics = data))
			.catch((error) => console.error('Failed to load relic data; rank editing disabled', error));
	});

	const extraStats = $derived(
		Object.keys(player.status_point_list ?? {})
			.filter((key) => !HERO_STATS.includes(key))
			.sort()
	);

	function maxRankFor(stat: string): number | undefined {
		return relics[relicKeyFor(stat)]?.max_rank;
	}

	function effectFor(stat: string): number | undefined {
		const entry = relics[relicKeyFor(stat)];
		const rank = player.status_point_list[stat];
		if (!entry || !rank || rank < 1 || rank > entry.effect_rate.length) return undefined;
		const effect = entry.effect_rate[rank - 1];
		// capture_power's effect rate is 0 at every rank: show the rank, no percentage.
		return effect > 0 ? effect : undefined;
	}

	async function updateExtraStat(stat: string) {
		const max = maxRankFor(stat);
		// No cap, no edit. Opening the slider without a `max` would let it fall back to
		// its default of 50 and write, say, sphere_homing = 50 when the real max_rank is
		// 4 -- exactly the invalid save state the cap exists to prevent. Relic data is
		// still loading, or its fetch failed; the button is disabled either way, and this
		// is the guard that makes the write impossible rather than merely unlikely.
		if (max === undefined) return;
		// @ts-ignore
		const result = await modal.showModal<number>(NumberSliderModal, {
			title: m.edit_entity({ entity: stat }),
			value: player.status_point_list[stat],
			min: 0,
			max
		});
		if (result === undefined || result === null) return;
		// The modal already clamps to `max`; this is a backstop.
		const value = Math.min(Math.max(result, 0), max);
		player.status_point_list[stat] = value;
		player.state = EntryState.MODIFIED;
	}

	async function updateStat(statType: string) {
		console.log('updateStat', statType);
		let title = '';
		let initialValue = 0;
		switch (statType) {
			case 'health':
				title = m.edit_entity({ entity: m.health() });
				initialValue = player.status_point_list.max_hp;
				break;
			case 'stamina':
				title = m.edit_entity({ entity: m.stamina() });
				initialValue = player.status_point_list.max_sp;
				break;
			case 'attack':
				title = m.edit_entity({ entity: m.attack() });
				initialValue = player.status_point_list.attack;
				break;
			case 'workSpeed':
				title = m.edit_entity({ entity: m.workspeed() });
				initialValue = player.status_point_list.work_speed;
				break;
			case 'weight':
				title = m.edit_entity({ entity: m.weight() });
				initialValue = player.status_point_list.weight;
				break;
		}
		// @ts-ignore
		const result = await modal.showModal<number[]>(NumberSliderModal, {
			title,
			value: initialValue
		});
		if (result) {
			console.log('result', result);
			switch (statType) {
				case 'health':
					player.status_point_list.max_hp = result;
					break;
				case 'stamina':
					player.status_point_list.max_sp = result;
					break;
				case 'attack':
					player.status_point_list.attack = result;
					break;
				case 'workSpeed':
					player.status_point_list.work_speed = result;
					break;
				case 'weight':
					player.status_point_list.weight = result;
					break;
			}
			player.state = EntryState.MODIFIED;
		}
	}

	function handleMaxPlayerStats(): void {
		player.status_point_list.max_hp = 50;
		player.status_point_list.max_sp = 50;
		player.status_point_list.attack = 50;
		player.status_point_list.work_speed = 50;
		player.status_point_list.weight = 50;

		player.stomach = 100;
		player.state = EntryState.MODIFIED;
	}

	$effect(() => {
		player.hp = health * 1000;
	});
</script>

{#snippet statButton(type: string, icon: string, label: string, value: number)}
	<button
		class="hover:ring-secondary-500 bg-surface-600/50 flex w-full items-center space-x-2 rounded-sm py-2 pr-2 hover:ring"
		onclick={() => updateStat(type)}
	>
		<img src={icon} alt={label} class="mx-2 h-6 w-6" />
		<span class="grow text-start">{label}</span>
		<span>{value.toLocaleString()}</span>
	</button>
{/snippet}

{#snippet rankButton(stat: string)}
	{@const max = maxRankFor(stat)}
	{@const effect = effectFor(stat)}
	<!-- Read-only until we know this rank's cap: the row still shows the current value,
	     but an edit that cannot be clamped must not be reachable. -->
	<button
		class="hover:ring-secondary-500 bg-surface-600/50 flex w-full items-center space-x-2 rounded-sm py-2 pr-2 hover:ring disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:ring-0"
		disabled={max === undefined}
		onclick={() => updateExtraStat(stat)}
	>
		<span class="grow pl-2 text-start">{stat}</span>
		{#if effect !== undefined}
			<span class="opacity-80">+{effect}%</span>
		{/if}
		<span>
			{player.status_point_list[stat]}{#if max !== undefined}<span class="opacity-60">/{max}</span
				>{/if}
		</span>
	</button>
{/snippet}

<div id="player-stats" class="flex flex-col items-end space-y-1">
	{@render statButton('health', staticIcons.hpIcon, m.health(), health)}
	{@render statButton('stamina', staticIcons.staminaIcon, m.stamina(), stamina)}
	{@render statButton('attack', staticIcons.attackIcon, m.attack(), attack)}
	{@render statButton('workSpeed', staticIcons.workSpeedIcon, m.workspeed(), workSpeed)}
	{@render statButton('weight', staticIcons.weightIcon, m.weight(), weight)}
	{#each extraStats as stat (stat)}
		{@render rankButton(stat)}
	{/each}
	<CornerDotButton id="max-player-stats" class="w-24" label={m.max()} onClick={handleMaxPlayerStats} />
</div>
