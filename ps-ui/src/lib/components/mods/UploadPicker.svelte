<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { Button, Progress } from '$components/ui';
	import { getModsState } from '$states';
	import type { UploadPurpose } from '$types';
	import * as m from '$i18n/messages';
	import { MAX_UPLOAD_BYTES } from '$lib/utils/modUpload';
	import { formatSize } from './modList';
	import { uploadErrorText } from './uploadText';

	let {
		targetId,
		purpose,
		accept,
		label,
		onUploaded
	}: {
		targetId: string;
		purpose: UploadPurpose;
		accept: string;
		label: string;
		onUploaded: (path: string) => void;
	} = $props();

	const modsState = getModsState();
	const inputId = $props.id();
	let file = $state.raw<File | null>(null);

	const status = $derived(
		modsState.uploadStatus?.purpose === purpose && modsState.uploadStatus.targetId === targetId
			? modsState.uploadStatus
			: null
	);
	const active = $derived(status !== null && status.stage !== 'done' && status.stage !== 'failed');
	const percent = $derived(
		status && status.size > 0 ? Math.floor((status.sent / status.size) * 100) : 0
	);
	const stageText = $derived.by(() => {
		if (!status) return '';
		if (status.stage === 'hashing') return m.mods_upload_hashing({ name: status.name });
		if (status.stage === 'finishing') return m.mods_upload_finishing({ name: status.name });
		return m.mods_upload_progress({ name: status.name, percent });
	});

	$effect(() => {
		const path = status?.stage === 'done' ? status.path : null;
		if (!path) return;
		untrack(() => {
			modsState.clearUpload();
			onUploaded(path);
		});
	});

	onMount(() => () => {
		const current = modsState.uploadStatus;
		if (current?.purpose === purpose && current.targetId === targetId) modsState.clearUpload();
	});
</script>

<div class="flex flex-col gap-2">
	<label for={inputId} class="text-sm">{label}</label>
	<input
		id={inputId}
		type="file"
		{accept}
		disabled={active}
		class="text-sm"
		onchange={(event) => (file = event.currentTarget.files?.[0] ?? null)}
	/>
	<p class="text-surface-500 text-xs">
		{m.mods_upload_hint({ max: formatSize(MAX_UPLOAD_BYTES) })}
	</p>
	{#if active && status}
		<div class="flex flex-col gap-1" role="group" aria-labelledby={`${inputId}-stage`}>
			<span id={`${inputId}-stage`} class="text-surface-300 text-xs" aria-live="polite">
				{stageText}
			</span>
			<Progress value={percent} max={100} showLabel={false} rounded="rounded-full" />
		</div>
		<Button variant="ghost" size="sm" class="self-start" onclick={() => modsState.clearUpload()}>
			{m.mods_upload_cancel()}
		</Button>
	{:else}
		<Button
			variant="secondary"
			size="sm"
			class="self-start"
			disabled={!file || modsState.uploading}
			onclick={() => file && modsState.startUpload(targetId, file, purpose)}
			data-modal-primary
		>
			{m.mods_upload_start()}
		</Button>
		{#if modsState.uploading}
			<p class="text-surface-400 text-xs">{m.mods_upload_busy()}</p>
		{/if}
	{/if}
	{#if status?.stage === 'failed' && status.error}
		<p class="text-error-400 text-sm" role="alert">{uploadErrorText(status.error)}</p>
	{/if}
</div>
