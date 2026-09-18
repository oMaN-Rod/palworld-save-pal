<script lang="ts" module>
	const compact = new Intl.NumberFormat(undefined, { notation: 'compact' });
</script>

<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { NexusModSummary } from '$types';
	import * as m from '$i18n/messages';
	import { formatSize } from './modList';
	import { nexusDateText } from './nexusText';
	import NexusThumb from './NexusThumb.svelte';

	let {
		mod,
		selected,
		installed,
		onDetails
	}: {
		mod: NexusModSummary;
		selected: boolean;
		installed: boolean;
		onDetails: (modId: number) => void;
	} = $props();

	const updated = $derived(nexusDateText(mod.updated_at));
	const size = $derived(mod.file_size === null ? null : formatSize(mod.file_size));
</script>

<button
	type="button"
	data-nexus-mod-id={mod.mod_id}
	aria-label={m.mods_discover_details({ name: mod.name })}
	onclick={() => onDetails(mod.mod_id)}
	class={[
		'bg-surface-900 group flex min-h-96 cursor-pointer flex-col overflow-hidden rounded-lg border text-left transition-colors',
		selected
			? 'border-primary-400 ring-primary-400 ring-1'
			: 'border-surface-800 hover:border-surface-600'
	]}
>
	<div class="relative aspect-video w-full overflow-hidden">
		<div class="h-full w-full transition-transform duration-200 group-hover:scale-105">
			<NexusThumb {mod} />
		</div>
		{#if installed || mod.adult_content}
			<div class="absolute top-2 left-2 flex flex-wrap gap-1">
				{#if installed}
					<span
						class="bg-success-500/85 rounded-xs px-1.5 py-0.5 text-[10px] font-medium text-black"
					>
						{m.mods_discover_installed()}
					</span>
				{/if}
				{#if mod.adult_content}
					<span
						class="bg-warning-500/85 rounded-xs px-1.5 py-0.5 text-[10px] font-medium text-black"
					>
						{m.mods_discover_adult_badge()}
					</span>
				{/if}
			</div>
		{/if}
	</div>

	<div class="divide-surface-800 flex min-w-0 flex-1 flex-col divide-y px-3 pt-3 pb-2">
		<div class="min-w-0 space-y-1 pb-2">
			<h3 class="line-clamp-2 leading-snug font-semibold wrap-break-word" title={mod.name}>
				{mod.name}
			</h3>
			{#if mod.author}
				<p class="truncate text-sm opacity-70">{m.mods_discover_by({ author: mod.author })}</p>
			{/if}
		</div>
		{#if mod.category || updated}
			<div class="flex items-center justify-between gap-2 py-2 text-sm opacity-70">
				{#if mod.category}
					<span class="truncate">{mod.category}</span>
				{/if}
				{#if updated}
					<span
						class="ml-auto flex shrink-0 items-center gap-1"
						title={m.mods_discover_updated({ date: updated })}
					>
						<Icon icon="tabler:history" size={14} />
						<time datetime={mod.updated_at}>{updated}</time>
					</span>
				{/if}
			</div>
		{/if}
		{#if mod.summary}
			<p class="line-clamp-3 pt-2 text-sm wrap-break-word opacity-70">{mod.summary}</p>
		{/if}
	</div>

	<div
		class="bg-surface-800 text-surface-300 mt-auto flex min-h-8 items-center gap-x-4 px-3 py-1.5 text-xs"
	>
		<span
			class="flex items-center gap-1"
			title={m.mods_discover_endorsements({ count: mod.endorsements.toLocaleString() })}
		>
			<Icon icon="tabler:thumb-up" size={14} />
			{compact.format(mod.endorsements)}
		</span>
		<span
			class="flex items-center gap-1"
			title={m.mods_discover_downloads({ count: mod.downloads.toLocaleString() })}
		>
			<Icon icon="tabler:download" size={14} />
			{compact.format(mod.downloads)}
		</span>
		{#if size}
			<span class="flex items-center gap-1" title={m.mods_discover_file_size({ size })}>
				<Icon icon="tabler:database" size={14} />
				{size}
			</span>
		{/if}
	</div>
</button>
