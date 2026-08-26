<script lang="ts">
	import Crosshair from '@lucide/svelte/icons/crosshair';
	import { mapToWorld } from '../utils';
	import Hover from './Hover.svelte';
	import * as m from '$i18n/messages';

	const coords = { ...mapToWorld(0, 0), z: 0 };
</script>

<Hover title={m.origin_point()} subtitle={m.map_coordinate_origin_point()} {coords}>
	{#snippet icon()}
		<Crosshair class="text-primary-500 h-4 w-4" />
	{/snippet}
</Hover>
