<script lang="ts">
	import { EntryState, type Player } from '$types';
	import { ASSET_DATA_PATH, staticIcons } from '$types/icons';
	import { getModalState } from '$states';
	import { NumberSliderModal } from '$components/modals';
	import { CornerDotButton, Tooltip } from '$components/ui';
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

	// Match in-game EPalRelicType order (not alphabetical).
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

	// `capture_power` maps to `capture_rate`; all other keys map 1:1.
	function statKeyFor(relicKey: string): string {
		return relicKey === 'capture_power' ? 'capture_rate' : relicKey;
	}

	// Single source for hero stat key + label used by UI/read/write.
	const HERO_STATS: Record<string, { key: string; label: () => string }> = {
		health: { key: 'max_hp', label: m.health },
		stamina: { key: 'max_sp', label: m.stamina },
		attack: { key: 'attack', label: m.attack },
		workSpeed: { key: 'work_speed', label: m.workspeed },
		weight: { key: 'weight', label: m.weight }
	};

	// Fixed cap for hero stats; relic caps come from relic data.
	const HERO_MAX = 50;

	// Empty while loading/failure; relic rank editing remains disabled without caps.
	let relics: Record<string, RelicRankData> = $state({});
	$effect(() => {
		relicData
			.getRelicData()
			.then((data) => (relics = data))
			.catch((error) => console.error('Failed to load relic data; rank editing disabled', error));
	});

	// Full relic set in game order; unresolved data keeps editors disabled.
	const relicStats = $derived(RELIC_ORDER.filter((key) => relics[key] !== undefined));

	// Missing key means rank 0.
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
		// `capture_power` has 0% effect at every rank.
		return effect > 0 ? effect : undefined;
	}

	async function updateRelicStat(relicKey: string) {
		const max = maxRankFor(relicKey);
		// Do not allow edits until the relic-specific cap is known.
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
		// Backstop clamp.
		player.status_point_list[statKeyFor(relicKey)] = Math.min(Math.max(result, 0), max);
		player.state = EntryState.MODIFIED;
	}

	async function updateStat(statType: string) {
		const stat = HERO_STATS[statType];
		if (!stat) return;
		// @ts-ignore
		const result = await modal.showModal<number>(NumberSliderModal, {
			title: m.edit_entity({ entity: stat.label() }),
			value: player.status_point_list[stat.key] ?? 0,
			min: 0,
			max: HERO_MAX
		});
		// `0` is valid, so only reject null/undefined.
		if (result === undefined || result === null) return;
		// Backstop clamp.
		player.status_point_list[stat.key] = Math.min(Math.max(result, 0), HERO_MAX);
		player.state = EntryState.MODIFIED;
	}

	function handleMaxPlayerStats(): void {
		for (const { key } of Object.values(HERO_STATS)) {
			player.status_point_list[key] = HERO_MAX;
		}
		// Max relics to each stat's own cap.
		for (const relicKey of relicStats) {
			const max = maxRankFor(relicKey);
			if (max === undefined) continue;
			player.status_point_list[statKeyFor(relicKey)] = max;
		}

		player.stomach = 100;
		player.state = EntryState.MODIFIED;
	}

	$effect(() => {
		player.hp = health * 1000;
	});
</script>

{#snippet statButton(type: string, icon: string, label: string, value: number)}
	<Tooltip {label}>
		<button
			class="hover:ring-secondary-500 bg-surface-600/50 flex w-full items-center space-x-2 rounded-sm py-2 pr-2 hover:ring"
			onclick={() => updateStat(type)}
		>
			<img src={icon} alt={label} class="mx-2 h-6 w-6" />
			<span>{value.toLocaleString()}</span>
		</button>
	</Tooltip>
{/snippet}

{#snippet relicButton(relicKey: string)}
	{@const entry = relics[relicKey]}
	{@const max = maxRankFor(relicKey)}
	{@const effect = effectFor(relicKey)}
	<!-- Disable editing until this relic's cap is known. -->
	<Tooltip label={`${entry.localized_name} +${effect}%`}>
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
			<span>
				{rankOf(relicKey)}{#if max !== undefined}<span class="opacity-60">/{max}</span>{/if}
			</span>
		</button>
	</Tooltip>
{/snippet}

<div id="player-stats" class="flex flex-col items-end space-y-1">
	<div class="grid w-full grid-cols-3 gap-2">
		{@render statButton('health', staticIcons.hpIcon, m.health(), health)}
		{@render statButton('stamina', staticIcons.staminaIcon, m.stamina(), stamina)}
		{@render statButton('attack', staticIcons.attackIcon, m.attack(), attack)}
		{@render statButton('workSpeed', staticIcons.workSpeedIcon, m.workspeed(), workSpeed)}
		{@render statButton('weight', staticIcons.weightIcon, m.weight(), weight)}
		{#each relicStats as relicKey (relicKey)}
			{@render relicButton(relicKey)}
		{/each}
	</div>
	<CornerDotButton
		id="max-player-stats"
		class="w-24"
		label={m.max()}
		onClick={handleMaxPlayerStats}
	/>
</div>
