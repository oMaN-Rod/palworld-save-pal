<script lang="ts">
	import { untrack } from 'svelte';
	import { getModsState, getToastState } from '$states';
	import { MessageType } from '$types';
	import * as m from '$i18n/messages';
	import { launchErrorText } from './launchText';

	let { targetId }: { targetId: string } = $props();

	const modsState = getModsState();
	const toasts = getToastState();

	const refusal = $derived(modsState.lastErrorFor(MessageType.GAME_LAUNCH, targetId));
	const launched = $derived(modsState.lastLaunch[targetId]);

	/** Each launch outcome is announced once, then cleared so a remount doesn't repeat it. */
	$effect(() => {
		if (!refusal) return;
		const text = launchErrorText(refusal);
		untrack(() => {
			toasts.add(text, undefined, 'error', 8000);
			modsState.clearLastError(MessageType.GAME_LAUNCH, targetId);
		});
	});

	$effect(() => {
		if (!launched) return;
		const profile =
			modsState.profiles[targetId]?.find((entry) => entry.id === launched.profile_id)?.name ??
			launched.profile_id;
		untrack(() => {
			toasts.add(m.mods_launch_started({ profile }), undefined, 'success');
			modsState.dismissLaunch(targetId);
		});
	});
</script>
