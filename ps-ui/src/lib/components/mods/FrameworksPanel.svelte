<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { untrack } from 'svelte';
	import { Button, Card } from '$components/ui';
	import { getModalState, getModsState } from '$states';
	import { MessageType } from '$types';
	import type { FrameworkEntry } from '$types';
	import * as m from '$i18n/messages';
	import { frameworkErrorText, frameworkLabels } from './frameworkText';
	import { displayName } from './modList';

	let { targetId }: { targetId: string } = $props();

	const modsState = getModsState();
	const modal = getModalState();
	const baseId = $props.id();

	const frameworks = $derived(modsState.frameworks[targetId]?.frameworks ?? []);
	const busy = $derived(modsState.frameworkBusy[targetId] ?? false);
	const statusError = $derived(modsState.lastErrorFor(MessageType.FRAMEWORK_STATUS, targetId));

	let selectedVersion = $state<Record<string, string>>({});

	$effect(() => {
		const id = targetId;
		void modsState.resets;
		untrack(() => modsState.loadFrameworks(id));
	});

	function installRefusalFor(framework: FrameworkEntry) {
		const install = modsState.lastErrorFor(MessageType.FRAMEWORK_INSTALL, targetId);
		return install?.key === framework.key ? install : undefined;
	}

	function removeRefusalFor(framework: FrameworkEntry) {
		const remove = modsState.lastErrorFor(MessageType.FRAMEWORK_REMOVE, targetId);
		return remove?.key === framework.key ? remove : undefined;
	}

	/** `dependents` mixes framework keys and mod ids; resolve each to the name it is shown by. */
	function dependentNames(dependents: string[]): string[] {
		return dependents.map((id) => {
			if (frameworkLabels[id]) return frameworkLabels[id];
			const mod = modsState.mods.find((entry) => entry.id === id);
			return mod ? displayName(mod) : id;
		});
	}

	function install(framework: FrameworkEntry) {
		modsState.installFramework(
			targetId,
			framework.key,
			selectedVersion[framework.key] || undefined
		);
	}

	async function requestRemove(framework: FrameworkEntry) {
		const confirmed = await modal.showConfirmModal({
			title: m.mods_framework_remove_title({ name: framework.name }),
			message:
				framework.key === 'ue4ss'
					? m.mods_framework_remove_confirm_ue4ss()
					: m.mods_framework_remove_confirm(),
			confirmText: m.mods_framework_remove({ name: framework.name }),
			cancelText: m.mods_panel_cancel()
		});
		if (!confirmed) return;
		modsState.removeFramework(targetId, framework.key);
	}

	function removeAnyway(framework: FrameworkEntry) {
		modsState.removeFramework(targetId, framework.key, true);
	}
</script>

<div class="flex flex-col gap-3">
	<div class="flex items-center justify-end">
		<Button
			variant="ghost"
			size="sm"
			class="flex items-center gap-2"
			disabled={busy}
			onclick={() => modsState.loadFrameworks(targetId, true)}
		>
			<Icon icon="tabler:refresh" size={14} />
			{m.mods_framework_check_updates()}
		</Button>
	</div>

	{#if statusError}
		<p class="text-error-400 text-sm" role="alert">{frameworkErrorText(statusError)}</p>
	{/if}

	{#each frameworks as framework (framework.key)}
		{@const installRefusal = installRefusalFor(framework)}
		{@const removeRefusal = removeRefusalFor(framework)}
		{@const refusal = installRefusal ?? removeRefusal}
		{@const versionId = `${baseId}-${framework.key}-version`}
		<div data-framework={framework.key}>
			<Card padding="p-3" class="flex flex-col gap-2">
				<div class="flex flex-wrap items-center justify-between gap-3">
					<div class="min-w-0">
						<p class="text-sm font-medium">{framework.name}</p>
						<p class="text-surface-400 text-xs">
							{framework.installed.present
								? m.mods_framework_installed_version({
										version: framework.installed.version ?? ''
									})
								: m.mods_framework_not_installed()}
						</p>
					</div>
					<div class="flex flex-wrap items-center gap-2 text-xs">
						{#if framework.installed.present}
							<span class="text-surface-400">
								{framework.installed.managed
									? m.mods_framework_managed()
									: m.mods_framework_unmanaged()}
							</span>
						{/if}
						{#if framework.latest}
							<span class="text-surface-400">
								{m.mods_framework_latest({ version: framework.latest.display })}
							</span>
						{:else if framework.latest_error}
							<span class="text-surface-500">{framework.latest_error.message}</span>
						{/if}
					</div>
				</div>

				{#if framework.library.length > 0}
					<div class="flex flex-wrap items-center gap-2 text-xs">
						<label for={versionId} class="text-surface-400">
							{m.mods_framework_version_label({ name: framework.name })}
						</label>
						<select
							id={versionId}
							class="bg-surface-800 border-surface-600 text-surface-100 focus:border-primary-500 rounded-lg border px-3 py-1.5 text-sm focus:outline-none disabled:opacity-60"
							value={selectedVersion[framework.key] ?? ''}
							disabled={busy}
							onchange={(event) => (selectedVersion[framework.key] = event.currentTarget.value)}
						>
							<option value="">{m.mods_framework_version_latest()}</option>
							{#each framework.library as entry (entry.mod_version_id)}
								<option value={entry.mod_version_id}>{entry.display}</option>
							{/each}
						</select>
					</div>
				{/if}

				<div class="flex flex-wrap gap-2">
					{#if framework.update_available}
						<Button size="sm" disabled={busy} onclick={() => install(framework)}>
							{m.mods_framework_update({ name: framework.name })}
						</Button>
					{:else if !framework.installed.present}
						<Button size="sm" disabled={busy} onclick={() => install(framework)}>
							{m.mods_framework_install({ name: framework.name })}
						</Button>
					{/if}
					{#if framework.installed.present && framework.installed.managed}
						<Button
							size="sm"
							variant="ghost"
							disabled={busy}
							onclick={() => requestRemove(framework)}
						>
							{m.mods_framework_remove({ name: framework.name })}
						</Button>
					{/if}
					{#if removeRefusal?.code === 'framework_required'}
						<Button
							size="sm"
							variant="ghost"
							disabled={busy}
							onclick={() => removeAnyway(framework)}
						>
							{m.mods_framework_remove_anyway({ name: framework.name })}
						</Button>
					{/if}
				</div>

				{#if refusal}
					{@const names =
						refusal.code === 'framework_required'
							? dependentNames((refusal.dependents as string[]) ?? [])
							: undefined}
					<p class="text-error-400 text-xs" role="alert">{frameworkErrorText(refusal, names)}</p>
				{/if}
			</Card>
		</div>
	{/each}
</div>
