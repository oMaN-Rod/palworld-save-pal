<script lang="ts">
	import { EntryState, type Pal } from '$types';
	import { Tooltip } from '$components/ui';
	import { NumberSliderModal } from '$components/modals';
	import { getAppState, getModalState } from '$states';
	import { staticIcons } from '$types/icons';
	import * as m from '$i18n/messages';

	type SoulKey = 'rank_hp' | 'rank_attack' | 'rank_defense' | 'rank_craftspeed';

	let {
		pal = $bindable()
	}: {
		pal: Pal;
	} = $props();

	const appState = getAppState();
	const modal = getModalState();

	const max = $derived(appState.settings.cheat_mode ? 255 : 20);
	const markers = $derived(appState.settings.cheat_mode ? [50, 100, 150, 200] : [5, 10, 15]);

	const souls = $derived([
		{
			key: 'rank_hp' as SoulKey,
			label: m.health(),
			icon: staticIcons.hpIcon,
			value: pal.rank_hp ?? 0
		},
		{
			key: 'rank_attack' as SoulKey,
			label: m.attack(),
			icon: staticIcons.attackIcon,
			value: pal.rank_attack ?? 0
		},
		{
			key: 'rank_defense' as SoulKey,
			label: m.defense(),
			icon: staticIcons.defenseIcon,
			value: pal.rank_defense ?? 0
		},
		{
			key: 'rank_craftspeed' as SoulKey,
			label: m.workspeed(),
			icon: staticIcons.workSpeedIcon,
			value: pal.rank_craftspeed ?? 0
		}
	]);

	async function handleEditSoul(key: SoulKey, label: string, value: number): Promise<void> {
		// @ts-ignore
		const result = await modal.showModal<number>(NumberSliderModal, {
			title: m.edit_entity({ entity: label }),
			value,
			min: 0,
			max,
			markers
		});
		if (result === null || result === undefined) return;
		pal[key] = result;
		pal.state = EntryState.MODIFIED;
	}
</script>

<div class="grid w-full grid-cols-4 gap-2">
	{#each souls as soul (soul.key)}
		<Tooltip>
			<button
				class="border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none"
				onclick={() => handleEditSoul(soul.key, soul.label, soul.value)}
			>
				<div class="flex w-full items-center">
					<img src={soul.icon} alt="{soul.label} icon" class="ml-2 h-4 w-4 2xl:h-6 2xl:w-6" />
					<span class="p-2 text-sm font-bold 2xl:text-lg">{soul.value}</span>
				</div>
			</button>
			{#snippet popup()}
				<div class="flex items-center space-x-2">
					<span class="text-lg font-bold">{soul.value}</span>
					<span class="text-lg">{soul.label}</span>
				</div>
			{/snippet}
		</Tooltip>
	{/each}
</div>
