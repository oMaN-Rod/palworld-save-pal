<script lang="ts">
	import { blueprintsData, type BlueprintFormat } from '$lib/data/blueprints.svelte';
	import {
		captureOptionsForPreset,
		CAPTURE_OPTION_FIELDS,
		type CapturePreset
	} from '$utils/blueprintOptions';
	import { getToastState } from '$states';
	import { Button, Card, Input, Checkbox, Select } from '$components/ui';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import type { CaptureOptions } from '$types';

	let { baseId, baseName, closeModal } = $props<{
		baseId: string;
		baseName: string;
		closeModal: (value: boolean | null) => void;
	}>();

	const toast = getToastState();

	let modalContainer: HTMLDivElement;
	let name = $state(baseName || 'Blueprint');
	let options = $state<CaptureOptions>(captureOptionsForPreset('blueprint'));
	let showAdvanced = $state(false);
	let toLibrary = $state(true);
	let toFile = $state(false);
	let fileFormat: BlueprintFormat = $state('psp');
	let busy = $state(false);

	function applyPreset(preset: CapturePreset) {
		options = captureOptionsForPreset(preset);
	}

	async function confirm() {
		if (!toLibrary && !toFile) {
			toast.add('Choose at least one destination.', 'Nothing to do', 'error');
			return;
		}
		busy = true;
		try {
			const { handle } = await blueprintsData.capture(baseId, options, name.trim() || 'Blueprint');
			if (toLibrary) await blueprintsData.store(handle);
			if (toFile) blueprintsData.exportFile(handle, fileFormat);
			toast.add(`Captured ${name}.`, 'Blueprint', 'success');
			closeModal(true);
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Capture failed', 'error');
		} finally {
			busy = false;
		}
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="max-w-lg min-w-[400px]">
		<h3 class="h3">Export blueprint</h3>

		<div class="mt-2 flex flex-col gap-2">
			<Input bind:value={name} label="Name" />

			<span class="label-text">Preset</span>
			<div class="flex gap-2">
				<Button variant="secondary" size="sm" onclick={() => applyPreset('blueprint')}
					>Blueprint</Button
				>
				<Button variant="secondary" size="sm" onclick={() => applyPreset('configured')}
					>Configured</Button
				>
				<Button variant="secondary" size="sm" onclick={() => applyPreset('full')}>Full</Button>
			</div>

			<button
				type="button"
				class="text-secondary-500 mt-1 self-start text-sm underline"
				onclick={() => (showAdvanced = !showAdvanced)}
			>
				{showAdvanced ? 'Hide' : 'Show'} advanced
			</button>

			{#if showAdvanced}
				<div class="bg-surface-800/50 flex flex-col gap-2 rounded-sm p-2">
					{#each CAPTURE_OPTION_FIELDS as field}
						<div>
							<Checkbox bind:checked={options[field.key]} label={field.label} />
							<p class="text-surface-400 ml-7 text-xs">{field.description}</p>
						</div>
					{/each}
				</div>
			{/if}

			<div class="mt-2 flex flex-col gap-1">
				<span class="label-text">Destination</span>
				<Checkbox bind:checked={toLibrary} label="Save to library" />
				<Checkbox bind:checked={toFile} label="Export to file" />
				{#if toFile}
					<Select
						label="Format"
						value={fileFormat}
						options={[
							{ value: 'psp', label: '.psp' },
							{ value: 'json', label: '.json' }
						]}
						onChange={(v) => (fileFormat = v as BlueprintFormat)}
					/>
				{/if}
			</div>
		</div>

		<div class="mt-4 flex justify-end gap-2">
			<Button variant="neutral" onclick={() => closeModal(null)}>Cancel</Button>
			<Button variant="primary" disabled={busy} onclick={confirm} data-modal-primary>
				Export
			</Button>
		</div>
	</Card>
</div>
