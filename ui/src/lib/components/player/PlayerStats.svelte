<script lang="ts">
	import { EntryState, type Player } from '$types';
	import { staticIcons } from '$types/icons';
	import { getModalState } from '$states';
	import { NumberSliderModal } from '$components/modals';
	import { CornerDotButton } from '$components/ui';
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

<div class="flex flex-col items-end space-y-1">
	{@render statButton('health', staticIcons.hpIcon, m.health(), health)}
	{@render statButton('stamina', staticIcons.staminaIcon, m.stamina(), stamina)}
	{@render statButton('attack', staticIcons.attackIcon, m.attack(), attack)}
	{@render statButton('workSpeed', staticIcons.workSpeedIcon, m.workspeed(), workSpeed)}
	{@render statButton('weight', staticIcons.weightIcon, m.weight(), weight)}
	<CornerDotButton class="w-24" label={m.max()} onClick={handleMaxPlayerStats} />
</div>
