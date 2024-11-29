<script lang="ts">
	import type { Player, SelectOption } from '$types';
	import { Select } from '$components/ui';
	import { getAppState } from '$states';

	let appState = getAppState();

	let { ...additionalProps } = $props<{
		[key: string]: any;
	}>();

	let selectOptions: SelectOption[] = $state([]);

	const label = $derived.by(() => {
		const worldName =
			appState.saveFile?.world_name === 'No LevelMeta.sav found'
				? undefined
				: appState.saveFile?.world_name;
		return worldName ? `Players in ${worldName}` : 'Players';
	});

	$effect(() => {
		selectOptions = Object.entries(appState.players as Record<string, Player>).map(
			([uid, player]) => ({
				value: uid,
				label: player.nickname || `Player ${uid}`
			})
		);
	});

	$effect(() => {
		appState.selectedPlayer = appState.players[appState.selectedPlayerUid];
	});
</script>

<div class="w-full" {...additionalProps}>
	<Select {label} options={selectOptions} bind:value={appState.selectedPlayerUid} />
</div>
