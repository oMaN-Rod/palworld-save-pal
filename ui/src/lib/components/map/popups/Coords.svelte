<script lang="ts">
	import type { WorldMapPoint } from '$types';
	import Globe from '@lucide/svelte/icons/globe';
	import MapIcon from '@lucide/svelte/icons/map';
	import { worldToMap } from '../utils';
	import InfoRow from './InfoRow.svelte';
	import * as m from '$i18n/messages';

	let { coords }: { coords: WorldMapPoint } = $props();

	const mapCoords = $derived(worldToMap(coords.x, coords.y));
	const world = $derived(
		[coords.x, coords.y, coords.z]
			.filter((n) => typeof n === 'number')
			.map((n) => n.toFixed(2))
			.join(', ')
	);
</script>

<InfoRow icon={Globe} label={m.world_coordinates()} value={world} />
<InfoRow icon={MapIcon} label={m.map_coordinates()} value="{mapCoords.x}, {mapCoords.y * -1}" />
