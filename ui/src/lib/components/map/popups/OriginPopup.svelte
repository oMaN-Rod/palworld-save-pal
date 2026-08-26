<script lang="ts">
	import Crosshair from '@lucide/svelte/icons/crosshair';
	import { mapToWorld } from '../utils';
	import Popup from './Popup.svelte';
	import * as m from '$i18n/messages';

	const coords = { ...mapToWorld(0, 0), z: 0 };
</script>

<Popup title={m.origin_point()} subtitle={m.map_coordinate_origin_point()} {coords}>
	{#snippet icon()}
		<Crosshair class="text-primary-500 h-5 w-5" />
	{/snippet}
</Popup>
