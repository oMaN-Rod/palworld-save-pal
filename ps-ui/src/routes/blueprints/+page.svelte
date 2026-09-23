<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import * as m from '$i18n/messages';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { blueprintsData, type BlueprintFormat } from '$lib/data/blueprints.svelte';
	import { placementState } from '$lib/data/placement.svelte';
	import { getAppState, getModalState, getToastState } from '$states';
	import { ExportBlueprintModal, SelectBaseModal } from '$components/modals';
	import { Button, Card, FileDropzone } from '$components/ui';
	import type { BlueprintFinding, BlueprintRow } from '$types';

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();

	const messages = m as unknown as Record<string, (() => string) | undefined>;

	function findingText(finding: BlueprintFinding): string {
		return (
			messages[`blueprint_finding_${finding.code.replaceAll('.', '_')}`]?.() ?? finding.message
		);
	}

	let importFiles: FileList | undefined = $state();

	onMount(() => {
		blueprintsData.list().catch((error) => {
			console.error('Error listing blueprints:', error);
		});
	});

	function loadedBases(): { id: string; name: string; guildName: string }[] {
		return Object.values(appState.guilds ?? {}).flatMap((guild) =>
			Object.values(guild.bases ?? {}).map((base) => ({
				id: base.id,
				name: base.name || base.id,
				guildName: guild.name
			}))
		);
	}

	async function captureNew() {
		const bases = loadedBases();
		// @ts-ignore  Component typing
		const picked = await modal.showModal<{ id: string; name: string } | null>(SelectBaseModal, {
			bases
		});
		if (!picked) return;
		// @ts-ignore
		await modal.showModal<boolean>(ExportBlueprintModal, {
			baseId: picked.id,
			baseName: picked.name
		});
	}

	$effect(() => {
		if (importFiles && importFiles.length > 0) {
			const files = importFiles;
			importFiles = undefined;
			importFile(files);
		}
	});

	async function importFile(files: FileList) {
		const file = files?.[0];
		if (!file) return;
		// Just a hint for compatibility; the server sniffs the content to decide the real format.
		const format: BlueprintFormat = file.name.toLowerCase().endsWith('.json') ? 'json' : 'psbp';
		const content = await fileToBase64(file);
		try {
			const res = await blueprintsData.loadFromContent(content, format, file.name);
			toast.add(`Imported ${res.header.name}.`, 'Blueprint', 'success');
			const save = await modal.showConfirmModal({
				title: `Save "${res.header.name}" to the library?`,
				confirmText: 'Save',
				cancelText: 'Skip'
			});
			if (save) await blueprintsData.store(res.handle);
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Import failed', 'error');
		}
	}

	function fileToBase64(file: File): Promise<string> {
		return new Promise((resolve, reject) => {
			const reader = new FileReader();
			reader.onload = () => {
				const result = reader.result as string;
				resolve(result.slice(result.indexOf(',') + 1));
			};
			reader.onerror = () => reject(reader.error);
			reader.readAsDataURL(file);
		});
	}

	function fmtDate(unixSeconds: number): string {
		return new Date(unixSeconds * 1000).toLocaleString();
	}

	async function placeRow(row: BlueprintRow) {
		try {
			const res = await blueprintsData.loadFromId(row.id);
			placementState.enter(res.handle, res.header);
			await goto('/map');
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Failed to load blueprint', 'error');
		}
	}

	async function exportRow(row: BlueprintRow, format: BlueprintFormat) {
		try {
			await blueprintsData.exportRow(row.id, format);
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Export failed', 'error');
		}
	}

	async function deleteRow(row: BlueprintRow) {
		const ok = await modal.showConfirmModal({
			title: `Delete blueprint "${row.name}"?`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});
		if (!ok) return;
		try {
			await blueprintsData.remove(row.id);
			toast.add(`Deleted ${row.name}.`, 'Blueprint', 'success');
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Delete failed', 'error');
		}
	}
</script>

<div class="flex flex-col gap-4 p-4">
	<div id="blueprints-header" class="flex flex-wrap items-center justify-between gap-2">
		<h1 class="text-xl font-semibold">Blueprints</h1>
		<div class="flex gap-2">
			<Button onclick={captureNew}>Capture new blueprint</Button>
		</div>
	</div>

	<FileDropzone name="blueprint-import" accept=".psbp,.psp,.json,.pstbase" bind:files={importFiles}>
		{#snippet message()}
			<h3 class="h3">Import a blueprint</h3>
			<span>{m.blueprint_import_dropzone_hint()}</span>
		{/snippet}
	</FileDropzone>

	{#if blueprintsData.lastImportFindings.length > 0}
		<div class="mt-2 space-y-1">
			{#each blueprintsData.lastImportFindings as finding (finding.code + finding.message)}
				<p class="text-sm opacity-80">{findingText(finding)}</p>
			{/each}
		</div>
	{/if}

	{#if blueprintsData.rows.length === 0}
		<p class="opacity-70">{m.blueprint_import_empty_state()}</p>
	{:else}
		<div class="flex max-h-100 flex-col gap-2 overflow-y-auto 2xl:max-h-164">
			{#each blueprintsData.rows as row (row.id)}
				<Card>
					<div
						id="blueprint-{row.id}"
						class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between"
					>
						<div class="min-w-0">
							<div class="truncate font-medium">{row.name}</div>
							<div class="truncate text-sm opacity-70">
								{row.source_world || 'unknown world'} · {row.structure_count} structures · {fmtDate(
									row.created_at
								)}
							</div>
						</div>
						<div id="blueprint-actions-{row.id}" class="flex flex-wrap gap-2 sm:shrink-0">
							<Button
								onclick={() => placeRow(row)}
								disabled={!appState.saveFile}
								title={appState.saveFile ? undefined : 'Load a save first'}
							>
								Place
							</Button>
							<Button variant="secondary" onclick={() => exportRow(row, 'psbp')}
								>Export .psbp</Button
							>
							<Button variant="ghost" onclick={() => exportRow(row, 'json')}>.json</Button>
							<Button variant="ghost" title="Delete" onclick={() => deleteRow(row)}>
								<Icon icon="tabler:trash-x" size={16} />
							</Button>
						</div>
					</div>
				</Card>
			{/each}
		</div>
	{/if}
</div>
