<script lang="ts">
	import type { Player } from '$types';
	import { Combobox } from '$components/ui';
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
</script>

<div class="w-full" {...additionalProps}>
	<Combobox
		value={selected}
		options={selectOptions}
		placeholder="Select Player"
		onChange={(value) => onselect(appState.players[value])}
	/>
</div>
