<script lang="ts">
	import { Input } from '$components/ui';
	import { palsData } from '$lib/data';
	import { assetLoader } from '$utils';
	import { cn } from '$theme';
	import * as m from '$i18n/messages';
	import AlertOctagon from '@lucide/svelte/icons/octagon-x';
	import AlertTriangle from '@lucide/svelte/icons/triangle-alert';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import ChevronUp from '@lucide/svelte/icons/chevron-up';
	import Search from '@lucide/svelte/icons/search';
	import type { OverviewStats } from '$states';

	let { anomalies }: { anomalies: OverviewStats['anomalies'] } = $props();

	/** The dashboard previews this many rows before the expand toggle. */
	const PREVIEW_ROWS = 25;

	let expanded = $state(false);
	let search = $state('');

	type FlaggedRow = OverviewStats['anomalies']['flagged'][number];

	const warningCount = $derived(anomalies.pal_count - anomalies.danger_count);

	const filtered = $derived.by(() => {
		const query = search.trim().toLowerCase();
		if (!query) return anomalies.flagged;
		return anomalies.flagged.filter(
			(row) =>
				row.character_id.toLowerCase().includes(query) ||
				row.character_key.toLowerCase().includes(query) ||
				(palsData.getByKey(row.character_key)?.localized_name ?? '')
					.toLowerCase()
					.includes(query) ||
				row.instance_id.toLowerCase().includes(query)
		);
	});

	const visible = $derived(expanded ? filtered : filtered.slice(0, PREVIEW_ROWS));
	const searchable = $derived(anomalies.flagged.length > PREVIEW_ROWS || search.length > 0);

	function reasonLabel(code: string): string {
		const message = (m as unknown as Record<string, ((...args: unknown[]) => string) | undefined>)[
			`overview_reason_${code.toLowerCase()}`
		];
		return message?.() ?? code;
	}

	function palIcon(row: FlaggedRow): string {
		const isPal = palsData.getByKey(row.character_key)?.is_pal ?? true;
		// Paldeck portrait icons (t_*_icon_normal), not the full-body renders.
		return assetLoader.loadMenuImage(row.character_key, isPal);
	}

	function palName(row: FlaggedRow): string {
		return palsData.getByKey(row.character_key)?.localized_name ?? row.character_key;
	}
</script>

<div
	class={cn(
		'card',
		anomalies.danger_count > 0
			? 'border-error-500/50 bg-error-500/10!'
			: 'border-warning-500/50 bg-warning-500/10!'
	)}
>
	<div class="mb-4 flex flex-wrap items-center gap-3">
		{#if anomalies.danger_count > 0}
			<AlertOctagon class="text-error-400 h-8 w-8 shrink-0" />
		{:else}
			<AlertTriangle class="text-warning-400 h-8 w-8 shrink-0" />
		{/if}
		<div class="min-w-0 flex-1">
			<h3 class="text-surface-400 text-xs font-semibold tracking-wider uppercase">
				{m.overview_needs_review()}
			</h3>
			<p class={cn('text-sm', anomalies.danger_count > 0 ? 'text-error-400' : 'text-warning-400')}>
				{m.overview_flagged_summary({
					count: anomalies.pal_count.toLocaleString(),
					danger: anomalies.danger_count.toLocaleString(),
					warning: warningCount.toLocaleString()
				})}
			</p>
		</div>
		{#if anomalies.pal_count > PREVIEW_ROWS}
			<button
				type="button"
				class="border-surface-600/60 text-surface-300 hover:border-primary-400/60 hover:text-surface-100 rounded-md border px-3 py-1.5 text-xs font-medium transition-colors"
				onclick={() => (expanded = !expanded)}
			>
				{expanded ? m.overview_show_less() : m.show_all()}
			</button>
		{/if}
	</div>

	{#if anomalies.by_code.length > 0}
		<div class="mb-4 flex flex-wrap gap-2">
			{#each anomalies.by_code as entry (entry.code)}
				<span
					class={cn(
						'rounded-full border px-2.5 py-0.5 text-xs font-medium',
						entry.code.startsWith('ILLEGAL_')
							? 'border-error-500/40 bg-error-500/10 text-error-300'
							: 'border-primary-500/40 bg-primary-500/10 text-primary-300'
					)}
				>
					{reasonLabel(entry.code)} ×{entry.count}
				</span>
			{/each}
		</div>
	{/if}

	{#if searchable}
		<div class="text-surface-500 relative mb-3">
			<Search size={14} class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2" />
			<Input
				type="text"
				placeholder={m.overview_search_flagged()}
				bind:value={search}
				inputClass="pl-9 w-full"
			/>
		</div>
	{/if}

	{#if filtered.length === 0}
		<div class="text-surface-500 flex items-center gap-2 py-4 text-sm">
			<Search size={16} />
			<span>{m.overview_no_flagged_match()}</span>
		</div>
	{:else}
		<ul class="divide-surface-700/60 max-h-96 divide-y overflow-y-auto">
			{#each visible as row (row.instance_id)}
				<li class="flex items-center gap-3 py-2">
					{#if row.severity === 'danger'}
						<AlertOctagon size={16} class="text-error-400 shrink-0" />
					{:else}
						<AlertTriangle size={16} class="text-warning-400 shrink-0" />
					{/if}
					<img
						src={palIcon(row)}
						alt={palName(row)}
						class="h-8 w-8 shrink-0 rounded-md object-contain"
						loading="lazy"
					/>
					<div class="min-w-0 flex-1">
						<div class="flex items-baseline gap-2">
							<span class="text-surface-100 truncate text-sm font-medium">
								{palName(row)}
							</span>
							<span class="text-surface-500 shrink-0 text-xs">
								{m.overview_lv({ level: row.level })}
							</span>
						</div>
						<div class="flex flex-wrap gap-1">
							{#each row.codes as code (code)}
								<span
									class={cn(
										'rounded px-1.5 py-0.5 text-[10px] font-medium',
										code.startsWith('ILLEGAL_')
											? 'bg-error-500/15 text-error-300'
											: 'bg-primary-500/15 text-primary-300'
									)}
								>
									{reasonLabel(code)}
								</span>
							{/each}
						</div>
					</div>
					<span
						class="text-surface-500 hidden shrink-0 font-mono text-[10px] sm:inline"
						title={row.instance_id}
					>
						{row.instance_id.slice(0, 8)}
					</span>
				</li>
			{/each}
		</ul>
		{#if !expanded && filtered.length > PREVIEW_ROWS}
			<button
				type="button"
				class="text-primary-400 hover:text-primary-300 mt-2 flex w-full items-center justify-center gap-1 text-xs font-medium"
				onclick={() => (expanded = true)}
			>
				<ChevronDown size={14} />
				{m.show_all()} ({filtered.length.toLocaleString()})
			</button>
		{:else if expanded && filtered.length > PREVIEW_ROWS}
			<button
				type="button"
				class="text-primary-400 hover:text-primary-300 mt-2 flex w-full items-center justify-center gap-1 text-xs font-medium"
				onclick={() => (expanded = false)}
			>
				<ChevronUp size={14} />
				{m.overview_show_less()}
			</button>
		{/if}
	{/if}
</div>
