<script module lang="ts">
	export interface ProfileNameResult {
		name: string;
		copy: boolean;
	}
</script>

<script lang="ts">
	import { untrack } from 'svelte';
	import { Button, Card, Input } from '$components/ui';
	import * as m from '$i18n/messages';

	let {
		title,
		confirmText,
		initialName = '',
		copyFrom,
		closeModal
	}: {
		title: string;
		confirmText: string;
		initialName?: string;
		copyFrom?: string;
		closeModal: (result: ProfileNameResult | null) => void;
	} = $props();

	const MAX_NAME_CHARS = 64;

	let name = $state(untrack(() => initialName));
	let copy = $state(false);
	const trimmed = $derived(name.trim());
	const length = $derived([...trimmed].length);
	const valid = $derived(length > 0 && length <= MAX_NAME_CHARS);
</script>

<Card class="flex w-[420px] max-w-full flex-col gap-4">
	<h3 class="h3">{title}</h3>
	<Input label={m.mods_profile_name_label()} bind:value={name} />
	{#if length > MAX_NAME_CHARS}
		<p class="text-warning-400 text-xs">{m.mods_profile_name_hint()}</p>
	{/if}
	{#if copyFrom !== undefined}
		<label class="flex w-fit cursor-pointer items-center gap-2 text-sm">
			<input type="checkbox" class="accent-primary-500 size-4" bind:checked={copy} />
			{m.mods_profile_copy({ name: copyFrom })}
		</label>
	{/if}
	<div class="flex justify-end gap-2">
		<Button variant="ghost" onclick={() => closeModal(null)}>{m.mods_panel_cancel()}</Button>
		<Button
			variant="primary"
			disabled={!valid}
			onclick={() => closeModal({ name: trimmed, copy })}
			data-modal-primary
		>
			{confirmText}
		</Button>
	</div>
</Card>
