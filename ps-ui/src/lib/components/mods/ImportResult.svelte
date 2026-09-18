<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { getModsState } from '$states';
	import type { ImportReply } from '$types';
	import * as m from '$i18n/messages';
	import { displayName } from './modList';
	import { disabledText, missingReasonText } from './shareText';

	let { result }: { result: ImportReply } = $props();

	const modsState = getModsState();

	function modName(modId: string): string {
		const mod = modsState.mods.find((entry) => entry.id === modId);
		return mod ? displayName(mod) : modId;
	}
</script>

{#snippet section(title: string, lines: string[])}
	{#if lines.length > 0}
		<div class="flex flex-col gap-1">
			<p class="text-surface-300 font-medium">{title}</p>
			<ul class="list-disc pl-5">
				{#each lines as line, index (index)}
					<li>{line}</li>
				{/each}
			</ul>
		</div>
	{/if}
{/snippet}

<div class="flex flex-col gap-3 text-sm" role="status">
	<p class="text-success-400 flex items-center gap-2">
		<Icon icon="tabler:check" size={14} class="shrink-0" />
		<span>{m.mods_import_done({ name: result.profile.name })}</span>
	</p>
	{#if result.pinned.length > 0}
		<p class="text-surface-300">{m.mods_import_pinned({ count: result.pinned.length })}</p>
	{/if}
	{@render section(m.mods_import_following_current(), result.following_current.map(modName))}
	{@render section(
		m.mods_import_installed(),
		result.installed.map((entry) => modName(entry.mod_id))
	)}
	{@render section(
		m.mods_import_missing(),
		result.missing.map((entry) =>
			m.mods_import_missing_entry({
				name: entry.name,
				version: entry.version,
				reason: missingReasonText(entry.reason)
			})
		)
	)}
	{@render section(
		m.mods_import_disabled(),
		result.disabled.map((entry) =>
			m.mods_import_disabled_entry({
				name: modName(entry.mod_id),
				reason: disabledText(entry.code)
			})
		)
	)}
	{@render section(
		m.mods_import_frameworks(),
		result.frameworks.map((entry) =>
			m.mods_import_framework_entry({
				framework: entry.framework,
				version: entry.version,
				state: entry.in_library
					? m.mods_import_framework_in_library()
					: m.mods_import_framework_missing()
			})
		)
	)}
</div>
