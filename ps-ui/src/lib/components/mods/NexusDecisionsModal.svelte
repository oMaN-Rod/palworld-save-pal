<script lang="ts">
	import { Button, Card } from '$components/ui';
	import { getNexusState } from '$states';
	import type { Decision, NexusDownloadReply } from '$types';
	import * as m from '$i18n/messages';
	import { joinList } from './applyOutcome';
	import { destinationLabel } from './modLabels';

	let { reply, closeModal }: { reply: NexusDownloadReply; closeModal: () => void } = $props();

	const nexusState = getNexusState();
	const decisions = $derived(reply.needs_decisions ?? []);

	function decisionText(decision: Decision): string {
		switch (decision.kind) {
			case 'multiple_ue4ss_roots':
				return m.mods_review_multiple_ue4ss_roots({ roots: joinList(decision.roots, 'unit') });
			case 'pak_destination':
				return m.mods_review_pak_destination({
					file: decision.file,
					destination: destinationLabel(decision.default)
				});
			case 'unplaced_files':
				return m.mods_review_unplaced_files({ files: joinList(decision.files, 'unit') });
			case 'name_conflict':
				return m.mods_review_name_conflict({ proposed: decision.proposed });
			case 'nexus_variant':
				return m.mods_review_nexus_variant();
		}
	}

	function acceptDefaults(): void {
		nexusState.startDownload({
			target_id: reply.target_id,
			mod_id: reply.nexus_mod_id,
			file_id: reply.file_id,
			accept_defaults: true
		});
		closeModal();
	}

	function cancel(): void {
		nexusState.clearDecisions();
		closeModal();
	}

	/** The archive stays on disk either way (server keeps it unless install succeeds); this just clears the UI's own hold. */
	$effect(() => {
		return () => nexusState.clearDecisions();
	});
</script>

<Card class="flex w-[480px] max-w-full flex-col gap-4">
	<h3 class="h3">{m.mods_discover_decisions_title()}</h3>
	<p class="text-surface-300 text-sm">
		{m.mods_discover_decisions_body({ file: reply.file_name ?? '' })}
	</p>
	<ul class="flex list-disc flex-col gap-1 pl-5 text-sm">
		{#each decisions as decision, index (index)}
			<li>{decisionText(decision)}</li>
		{/each}
	</ul>
	<div class="flex justify-end gap-2">
		<Button variant="ghost" onclick={cancel}>{m.mods_discover_decisions_cancel()}</Button>
		<Button variant="primary" onclick={acceptDefaults} data-modal-primary>
			{m.mods_discover_decisions_accept()}
		</Button>
	</div>
</Card>
