<script lang="ts">
	import type { Player } from '$types';
	import { Select } from '$components/ui';
	import { getAppState } from '$states';

	let appState = getAppState();

	let { selected, onselect, ...additionalProps } = $props<{
		selected?: string;
		onselect: (player: Player) => void;
		[key: string]: any;
	}>();

	const selectOptions = $derived(
		Object.entries(appState.players as Record<string, Player>).map(([uid, player]) => ({
			value: uid,
			label: player.nickname || `Player ${uid}`
		}))
	);

	if (Object.keys(appState.players).length === 1) {
        const singlePlayerId = Object.keys(appState.players)[0];
        selected = singlePlayerId;
        onselect(appState.players[singlePlayerId]);
    }
</script>

<div class="w-full" {...additionalProps}>
	<Select
		value={selected}
		options={selectOptions}
		placeholder="Select Player"
		onChange={(value) => onselect(appState.players[value])}
	/>
</div>
