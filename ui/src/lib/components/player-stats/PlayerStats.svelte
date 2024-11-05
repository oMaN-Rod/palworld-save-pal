<script lang="ts">
	import { EntryState, type Player } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { getModalState } from '$states';
	import { PlayerStatModal } from '$components/modals';
	import { CornerDotButton } from '$components/ui';

	let { player = $bindable() } = $props<{
		player: Player;
	}>();

	const modal = getModalState();

	let hpIcon: string = $state('');
	let staminaIcon: string = $state('');
	let attackIcon: string = $state('');
	let captureIcon: string = $state('');
	let workSpeedIcon: string = $state('');
	let weightIcon: string = $state('');

	let health = $derived(500 + player.status_point_list.max_hp * 100);
	let stamina = $derived(100 + player.status_point_list.max_sp * 10);
	let attack = $derived(100 + player.status_point_list.attack * 2);
	let workSpeed = $derived(100 + player.status_point_list.work_speed * 50);
	let weight = $derived(300 + player.status_point_list.weight * 50);

	async function loadStaticIcons() {
		const hpPath = `${ASSET_DATA_PATH}/img/stats/health.png`;
		const hp = await assetLoader.loadImage(hpPath);
		hpIcon = hp;

		const staminaPath = `${ASSET_DATA_PATH}/img/stats/stamina.png`;
		const stamina = await assetLoader.loadImage(staminaPath);
		staminaIcon = stamina;

		const attackPath = `${ASSET_DATA_PATH}/img/stats/attack.png`;
		const attack = await assetLoader.loadImage(attackPath);
		attackIcon = attack;

		const capturePath = `${ASSET_DATA_PATH}/img/stats/capture.png`;
		const capture = await assetLoader.loadImage(capturePath);
		captureIcon = capture;

		const workPath = `${ASSET_DATA_PATH}/img/stats/work_speed.png`;
		const work = await assetLoader.loadImage(workPath);
		workSpeedIcon = work;

		const weightPath = `${ASSET_DATA_PATH}/img/stats/weight.png`;
		const weight = await assetLoader.loadImage(weightPath);
		weightIcon = weight;
	}

	$effect(() => {
		loadStaticIcons();
	});

	async function updateStat(statType: string) {
		console.log('updateStat', statType);
		let title = '';
		let initialValue = 0;
		switch (statType) {
			case 'health':
				title = 'Edit Health';
				initialValue = player.status_point_list.max_hp;
				break;
			case 'stamina':
				title = 'Edit Stamina';
				initialValue = player.status_point_list.max_sp;
				break;
			case 'attack':
				title = 'Edit Attack';
				initialValue = player.status_point_list.attack;
				break;
			case 'workSpeed':
				title = 'Edit Work Speed';
				initialValue = player.status_point_list.work_speed;
				break;
			case 'weight':
				title = 'Edit Weight';
				initialValue = player.status_point_list.weight;
				break;
		}
		// @ts-ignore
		const result = await modal.showModal<number[]>(PlayerStatModal, { title, value: initialValue });
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
		class="hover:ring-secondary-500 bg-surface-600/50 flex w-full items-center space-x-2 rounded py-2 pr-2 hover:ring"
		onclick={() => updateStat(type)}
	>
		{#if icon}
			<enhanced:img src={icon} alt={label} class="mx-2 h-6 w-6" />
		{/if}
		<span class="grow text-start">{label}</span>
		<span>{value.toLocaleString()}</span>
	</button>
{/snippet}

<div class="flex flex-col items-end space-y-1">
	{@render statButton('health', hpIcon, 'Health', health)}
	{@render statButton('stamina', staminaIcon, 'Stamina', stamina)}
	{@render statButton('attack', attackIcon, 'Attack', attack)}
	{@render statButton('workSpeed', workSpeedIcon, 'Work Speed', workSpeed)}
	{@render statButton('weight', weightIcon, 'Weight', weight)}
	<CornerDotButton class="w-24" label="Max" onClick={handleMaxPlayerStats} />
</div>
