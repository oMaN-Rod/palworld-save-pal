<script lang="ts">
	import { EntryState, type Player } from '$types';
	import { ASSET_DATA_PATH, staticIcons } from '$types/icons';
	import { getModalState } from '$states';
	import { NumberSliderModal } from '$components/modals';
	import { CornerDotButton } from '$components/ui';
	import { relicData } from '$lib/data';
	import type { RelicRankData } from '$lib/data/relic.svelte';
	import { assetLoader } from '$utils';
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

	// Game order (the EPalRelicType enum order), which is the order the game's own
	// Buildup menu uses. NOT alphabetical.
	const RELIC_ORDER = [
		'capture_power',
		'hunger_reduction',
		'swim_speed',
		'food_decay_reduction',
		'jump_power',
		'glider_speed',
		'climb_speed',
		'status_ailment_resist',
		'stamina_reduction',
		'sphere_homing',
		'exp_bonus',
		'rainbow_passive_rate',
		'move_speed'
	];

	// The status stat is `capture_rate`; its relic type is `capture_power`. Every other
	// relic key IS the status key.
	function statKeyFor(relicKey: string): string {
		return relicKey === 'capture_power' ? 'capture_rate' : relicKey;
	}

	// Stays `{}` while the fetch is in flight, and if it fails. Either way we do not
	// know any rank's cap, and every rank editor stays disabled until we do -- see
	// `updateRelicStat`.
	let relics: Record<string, RelicRankData> = $state({});
	$effect(() => {
		relicData
			.getRelicData()
			.then((data) => (relics = data))
			.catch((error) => console.error('Failed to load relic data; rank editing disabled', error));
	});

	// All 13 categories, always -- not just the ones the save has a row for. A stat with
	// no row is a stat at rank 0; the backend appends the row when it is first raised
	// above 0. Empty until relic data loads, which keeps every editor disabled.
	const relicStats = $derived(RELIC_ORDER.filter((key) => relics[key] !== undefined));

	// The reader omits rows the save does not have, so an absent key means rank 0.
	function rankOf(relicKey: string): number {
		return player.status_point_list[statKeyFor(relicKey)] ?? 0;
	}

	function maxRankFor(relicKey: string): number | undefined {
		return relics[relicKey]?.max_rank;
	}

	function effectFor(relicKey: string): number | undefined {
		const entry = relics[relicKey];
		const rank = rankOf(relicKey);
		if (!entry || rank < 1 || rank > entry.effect_rate.length) return undefined;
		const effect = entry.effect_rate[rank - 1];
		// capture_power's effect rate is 0 at every rank: show the rank, no percentage.
		return effect > 0 ? effect : undefined;
	}

	async function updateRelicStat(relicKey: string) {
		const max = maxRankFor(relicKey);
		// No cap, no edit. Opening the slider without a `max` would let it fall back to
		// its default of 50 and write, say, sphere_homing = 50 when the real max_rank is
		// 4 -- exactly the invalid save state the cap exists to prevent. Relic data is
		// still loading, or its fetch failed; the button is disabled either way, and this
		// is the guard that makes the write impossible rather than merely unlikely.
		if (max === undefined) return;
		const entry = relics[relicKey];
		// @ts-ignore
		const result = await modal.showModal<number>(NumberSliderModal, {
			title: m.edit_entity({ entity: entry.localized_name }),
			value: rankOf(relicKey),
			min: 0,
			max
		});
		if (result === undefined || result === null) return;
		// The modal already clamps to `max`; this is a backstop.
		player.status_point_list[statKeyFor(relicKey)] = Math.min(Math.max(result, 0), max);
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

{#snippet relicButton(relicKey: string)}
	{@const entry = relics[relicKey]}
	{@const max = maxRankFor(relicKey)}
	{@const effect = effectFor(relicKey)}
	<!-- An edit that cannot be clamped must not be reachable, so the button is disabled
	     without a known cap. (Rows are driven by the relic data, so in practice a row
	     without a cap does not render at all -- this guard is the backstop, not the
	     mechanism.) -->
	<button
		class="hover:ring-secondary-500 bg-surface-600/50 flex w-full items-center space-x-2 rounded-sm py-2 pr-2 hover:ring disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:ring-0"
		disabled={max === undefined}
		title={entry.description}
		onclick={() => updateRelicStat(relicKey)}
	>
		<img
			src={assetLoader.loadImage(`${ASSET_DATA_PATH}/img/relic_${relicKey}.webp`)}
			alt={entry.localized_name}
			class="mx-2 h-6 w-6"
		/>
		<span class="grow text-start">{entry.localized_name}</span>
		{#if effect !== undefined}
			<span class="opacity-80">+{effect}%</span>
		{/if}
		<span>
			{rankOf(relicKey)}{#if max !== undefined}<span class="opacity-60">/{max}</span>{/if}
		</span>
	</button>
{/snippet}

<div id="player-stats" class="flex flex-col items-end space-y-1">
	{@render statButton('health', staticIcons.hpIcon, m.health(), health)}
	{@render statButton('stamina', staticIcons.staminaIcon, m.stamina(), stamina)}
	{@render statButton('attack', staticIcons.attackIcon, m.attack(), attack)}
	{@render statButton('workSpeed', staticIcons.workSpeedIcon, m.workspeed(), workSpeed)}
	{@render statButton('weight', staticIcons.weightIcon, m.weight(), weight)}
	{#each relicStats as relicKey (relicKey)}
		{@render relicButton(relicKey)}
	{/each}
	<CornerDotButton id="max-player-stats" class="w-24" label={m.max()} onClick={handleMaxPlayerStats} />
</div>
