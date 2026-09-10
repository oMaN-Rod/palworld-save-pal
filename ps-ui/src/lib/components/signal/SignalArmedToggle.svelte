<script lang="ts">
	import * as m from '$i18n/messages';
	import { getSignalState } from '$states';
	import { Switch } from '@skeletonlabs/skeleton-svelte';

	const signalState = getSignalState();

	let pendingArmed = $state<boolean | null>(null);
	let armError = $state<string | null>(null);

	const displayedArmed = $derived(pendingArmed ?? signalState.armed);

	async function toggleArmed() {
		const next = !displayedArmed;
		pendingArmed = next;
		armError = null;
		try {
			await signalState.setArmed(next);
		} catch (error) {
			armError = error instanceof Error ? error.message : String(error);
		} finally {
			pendingArmed = null;
		}
	}
</script>

<div class="flex flex-col gap-1">
	<div class="flex items-center gap-3">
		<Switch checked={displayedArmed} onCheckedChange={toggleArmed} />
		<span class="text-sm">{m.signal_remote_access_arm_label()}</span>
	</div>
	{#if armError}
		<p class="text-error-400 text-sm">{armError}</p>
	{:else if !displayedArmed}
		<p class="text-surface-400 text-xs">{m.signal_remote_access_disarmed_note()}</p>
	{/if}
</div>
