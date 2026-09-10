<script lang="ts">
	import { EntryState, type Pal } from '$types';
	import { Tooltip } from '$components/ui';
	import { NumberSliderModal } from '$components/modals';
	import { getAppState, getModalState } from '$states';
	import { staticIcons } from '$types/icons';
	import * as m from '$i18n/messages';

	type TalentKey = 'talent_hp' | 'talent_shot' | 'talent_defense';

	let {
		pal = $bindable()
	}: {
		pal: Pal;
	} = $props();

	const appState = getAppState();
	const modal = getModalState();

	const max = $derived(appState.settings.cheat_mode ? 255 : 100);
	const markers = $derived(appState.settings.cheat_mode ? [50, 100, 150, 200] : [25, 50, 75]);

	const talents = $derived([
		{
			key: 'talent_hp' as TalentKey,
			label: m.hp(),
			icon: staticIcons.hpIcon,
			value: pal.talent_hp ?? 0
		},
		{
			key: 'talent_shot' as TalentKey,
			label: m.attack(),
			icon: staticIcons.attackIcon,
			value: pal.talent_shot ?? 0
		},
		{
			key: 'talent_defense' as TalentKey,
			label: m.defense(),
			icon: staticIcons.defenseIcon,
			value: pal.talent_defense ?? 0
		}
	]);

	async function handleEditTalent(key: TalentKey, label: string, value: number): Promise<void> {
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

<div class="grid w-full grid-cols-3 gap-2">
	{#each talents as talent (talent.key)}
		<Tooltip>
			<button
				class="border-l-surface-600 bg-surface-900 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none"
				onclick={() => handleEditTalent(talent.key, talent.label, talent.value)}
			>
				<div class="flex w-full items-center">
					<img src={talent.icon} alt="{talent.label} icon" class="ml-2 h-4 w-4 2xl:h-6 2xl:w-6" />
					<span class="p-2 text-sm font-bold 2xl:text-lg">{talent.value}</span>
				</div>
			</button>
			{#snippet popup()}
				<div class="flex items-center space-x-2">
					<span class="text-lg font-bold">{talent.value}</span>
					<span class="text-lg">{talent.label}</span>
				</div>
			{/snippet}
		</Tooltip>
	{/each}
</div>
