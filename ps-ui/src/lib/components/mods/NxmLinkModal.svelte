<script lang="ts">
	import { Button, Card, Select } from '$components/ui';
	import { getModsState, getNexusState } from '$states';
	import type { NexusLinkPush } from '$types';
	import * as m from '$i18n/messages';

	let {
		payload,
		defaultTargetId,
		closeModal
	}: {
		payload: NexusLinkPush;
		defaultTargetId?: string;
		closeModal: () => void;
	} = $props();

	const modsState = getModsState();
	const nexusState = getNexusState();

	const link = $derived(payload.link);
	const usable = $derived(link !== undefined && payload.error === undefined);

	/** Stable sort keeps each kind's own order; clients just come first. */
	const orderedTargets = $derived(
		[...modsState.targets].sort((a, b) => {
			if (a.kind === b.kind) return 0;
			return a.kind === 'client' ? -1 : 1;
		})
	);

	let chosen = $state('');

	$effect(() => {
		if (chosen || orderedTargets.length === 0) return;
		const preferred = orderedTargets.find((target) => target.id === defaultTargetId);
		chosen = (preferred ?? orderedTargets[0]).id;
	});

	function errorSentence(): string {
		const code = payload.error?.code;
		if (code === 'link_expired') return m.mods_nxm_expired();
		if (code === 'unsupported_link') return m.mods_nxm_unsupported();
		return m.mods_nxm_invalid();
	}

	function download(): void {
		if (!link || !chosen) return;
		const paired = link.key !== null && link.expires !== null;
		nexusState.startDownload({
			target_id: chosen,
			mod_id: link.mod_id,
			file_id: link.file_id,
			key: paired ? (link.key ?? undefined) : undefined,
			expires: paired ? (link.expires ?? undefined) : undefined
		});
		closeModal();
	}
</script>

<Card class="flex w-[440px] max-w-full flex-col gap-4">
	<h3 class="h3">{m.mods_nxm_title()}</h3>

	{#if usable && link}
		{#if orderedTargets.length === 0}
			<p class="text-surface-300 text-sm">{m.mods_nxm_no_targets()}</p>
			<div class="flex justify-end">
				<Button variant="ghost" onclick={() => closeModal()}>{m.mods_nxm_dismiss()}</Button>
			</div>
		{:else}
			<p class="text-surface-300 text-sm">
				{m.mods_nxm_body({ file: link.file_id, mod: link.mod_id })}
			</p>
			<Select
				label={m.mods_nxm_target()}
				options={orderedTargets.map((target) => ({ value: target.id, label: target.name }))}
				value={chosen}
				onChange={(value) => (chosen = String(value))}
			/>
			<div class="flex justify-end gap-2">
				<Button variant="ghost" onclick={() => closeModal()}>{m.mods_nxm_dismiss()}</Button>
				<Button variant="primary" onclick={download} data-modal-primary>
					{m.mods_nxm_download()}
				</Button>
			</div>
		{/if}
	{:else}
		<p class="text-surface-300 text-sm" role="alert">{errorSentence()}</p>
		<div class="flex justify-end">
			<Button variant="ghost" onclick={() => closeModal()}>{m.mods_nxm_dismiss()}</Button>
		</div>
	{/if}
</Card>
