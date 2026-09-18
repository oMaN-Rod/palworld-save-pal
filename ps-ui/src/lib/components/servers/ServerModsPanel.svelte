<script lang="ts">
	import type { Server } from '$types';
	import { untrack } from 'svelte';
	import { TargetPanel, type ModsTab } from '$components/mods';
	import { getModsState } from '$states';

	let { server, tab = $bindable('mods') }: { server: Server; tab?: ModsTab } = $props();

	const modsState = getModsState();
	const targetId = $derived(`server-${server.id}`);
	let seenResets: number | undefined;

	/** While the target is missing, the panel reloads the targets and library itself. */
	$effect(() => {
		const resets = modsState.resets;
		untrack(() => {
			const reloaded = seenResets !== undefined && resets !== seenResets;
			seenResets = resets;
			if (reloaded && modsState.targets.some((target) => target.id === targetId)) {
				modsState.loadTargets();
				modsState.loadLibrary();
			}
		});
	});
</script>

<TargetPanel {targetId} bind:tab />
