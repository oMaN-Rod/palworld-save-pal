<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { tick } from 'svelte';
	import { Button } from '$components/ui';
	import { getModsState } from '$states';
	import { MessageType } from '$types';
	import type { RecordedModError, ReorderKind, Ue4ssControlMode } from '$types';
	import * as m from '$i18n/messages';
	import InactiveProfileNotice from './InactiveProfileNotice.svelte';
	import { alphabeticalRows, libraryCovers, moveId, orderRows, type OrderRow } from './loadOrder';
	import { displayName } from './modList';
	import { profileErrorText } from './profileText';

	let { targetId }: { targetId: string } = $props();

	const modsState = getModsState();
	const baseId = $props.id();

	const profile = $derived(modsState.viewedProfile(targetId));
	const busy = $derived(modsState.ordering[targetId] ?? false);
	const covered = $derived(profile ? libraryCovers(profile, modsState.mods) : false);
	const sections = $derived.by(() => {
		if (!profile) return [];
		return [
			{
				kind: 'ue4ss' as ReorderKind,
				title: m.mods_order_ue4ss_title(),
				hint: m.mods_order_ue4ss_hint(),
				rows: orderRows(profile, modsState.mods, 'ue4ss')
			},
			{
				kind: 'palschema' as ReorderKind,
				title: m.mods_order_palschema_title(),
				hint: m.mods_order_palschema_hint(),
				rows: orderRows(profile, modsState.mods, 'palschema')
			}
		];
	});
	const alphabetical = $derived(
		profile ? alphabeticalRows(profile, modsState.mods, ['pak', 'logicmods']) : []
	);
	const refusals = $derived(
		[MessageType.PROFILE_REORDER, MessageType.PROFILE_SET_OPTIONS]
			.map((type) => ({ type, error: modsState.lastErrorFor(type, targetId) }))
			.filter(
				(entry): entry is { type: MessageType; error: RecordedModError } =>
					entry.error !== undefined
			)
	);

	async function move(kind: ReorderKind, rows: OrderRow[], index: number, delta: -1 | 1) {
		if (!profile || busy || !covered) return;
		const ids = moveId(
			rows.map((row) => row.entry.mod_id),
			index,
			delta
		);
		if (!ids) return;
		const movedId = rows[index].entry.mod_id;
		modsState.reorderProfile(targetId, profile.id, kind, ids);
		await tick();
		[...document.querySelectorAll<HTMLElement>(`[data-order-kind="${kind}"]`)]
			.find((element) => element.dataset.modId === movedId)
			?.focus();
	}

	function onRowKey(event: KeyboardEvent, kind: ReorderKind, rows: OrderRow[], index: number) {
		if (!event.altKey || (event.key !== 'ArrowUp' && event.key !== 'ArrowDown')) return;
		event.preventDefault();
		void move(kind, rows, index, event.key === 'ArrowUp' ? -1 : 1);
	}

	function setOptions(options: Parameters<typeof modsState.setProfileOptions>[2]) {
		if (profile) modsState.setProfileOptions(targetId, profile.id, options);
	}
</script>

{#if profile}
	<div class="flex flex-col gap-4">
		<InactiveProfileNotice {profile} />

		{#each refusals as { type, error } (type)}
			<p class="text-error-400 flex items-center gap-2 text-sm" role="alert">
				<Icon icon="tabler:alert-circle" size={14} class="shrink-0" />
				{profileErrorText(error)}
			</p>
		{/each}

		{#if !covered}
			<p class="text-surface-400 text-sm" role="status">{m.mods_order_library_loading()}</p>
		{/if}

		<p class="text-surface-400 text-xs">{m.mods_order_keyboard_hint()}</p>

		{#each sections as section (section.kind)}
			<section class="flex flex-col gap-2">
				<h4 id={`${baseId}-${section.kind}`} class="text-sm font-medium">{section.title}</h4>
				<p class="text-surface-400 text-xs">{section.hint}</p>
				{#if section.rows.length === 0}
					<p class="text-surface-500 text-sm">{m.mods_order_empty()}</p>
				{:else}
					<ol class="flex flex-col gap-1" aria-labelledby={`${baseId}-${section.kind}`}>
						{#each section.rows as row, index (row.entry.mod_id)}
							<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
							<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
							<li
								data-order-kind={section.kind}
								data-mod-id={row.entry.mod_id}
								tabindex="0"
								class="bg-surface-800 focus:ring-primary-500 flex items-center justify-between gap-2 rounded-sm px-3 py-2 text-sm focus:ring-2 focus:outline-none"
								onkeydown={(event) => onRowKey(event, section.kind, section.rows, index)}
							>
								<span class="flex min-w-0 items-center gap-2">
									<span class="text-surface-500 w-6 text-right font-mono text-xs">{index + 1}</span>
									<span class="truncate {row.entry.enabled ? '' : 'text-surface-500'}">
										{displayName(row.mod)}
									</span>
									{#if !row.entry.enabled}
										<span class="text-surface-500 text-xs">{m.mods_order_disabled_entry()}</span>
									{/if}
								</span>
								<span class="flex shrink-0 items-center gap-1">
									<Button
										variant="ghost"
										size="sm"
										aria-label={m.mods_order_move_up({ name: displayName(row.mod) })}
										disabled={busy || !covered || index === 0}
										onclick={() => move(section.kind, section.rows, index, -1)}
									>
										<Icon icon="tabler:arrow-up" size={14} />
									</Button>
									<Button
										variant="ghost"
										size="sm"
										aria-label={m.mods_order_move_down({ name: displayName(row.mod) })}
										disabled={busy || !covered || index === section.rows.length - 1}
										onclick={() => move(section.kind, section.rows, index, 1)}
									>
										<Icon icon="tabler:arrow-down" size={14} />
									</Button>
								</span>
							</li>
						{/each}
					</ol>
				{/if}
			</section>
		{/each}

		<section class="flex flex-col gap-2">
			<h4 id={`${baseId}-alpha`} class="text-sm font-medium">
				{m.mods_order_alphabetical_title()}
			</h4>
			<p class="text-surface-400 text-xs">{m.mods_order_alphabetical_hint()}</p>
			{#if alphabetical.length === 0}
				<p class="text-surface-500 text-sm">{m.mods_order_empty()}</p>
			{:else}
				<ul class="flex flex-col gap-1" aria-labelledby={`${baseId}-alpha`}>
					{#each alphabetical as row (row.entry.mod_id)}
						<li class="bg-surface-800 rounded-sm px-3 py-2 text-sm">{displayName(row.mod)}</li>
					{/each}
				</ul>
			{/if}
		</section>

		<section class="flex flex-col gap-2">
			<h4 class="text-sm font-medium">{m.mods_order_options_title()}</h4>
			<label class="flex w-fit items-center gap-2 text-sm">
				<input
					type="checkbox"
					class="accent-primary-500 size-4"
					checked={profile.force_order_palschema}
					disabled={busy}
					onchange={(event) => setOptions({ force_order_palschema: event.currentTarget.checked })}
				/>
				{m.mods_order_force_palschema()}
			</label>
			<p class="text-surface-400 text-xs">{m.mods_order_force_palschema_hint()}</p>
			<label class="flex w-fit items-center gap-2 text-sm">
				<input
					type="checkbox"
					class="accent-primary-500 size-4"
					checked={profile.force_order_ue4ss}
					disabled={busy}
					onchange={(event) => setOptions({ force_order_ue4ss: event.currentTarget.checked })}
				/>
				{m.mods_order_force_ue4ss()}
			</label>
			<p class="text-surface-400 text-xs">{m.mods_order_force_ue4ss_hint()}</p>
			<div class="flex items-center gap-2 text-sm">
				<label for={`${baseId}-mode`}>{m.mods_order_control_mode()}</label>
				<select
					id={`${baseId}-mode`}
					class="bg-surface-800 border-surface-600 text-surface-100 focus:border-primary-500 rounded-lg border px-3 py-1.5 text-sm focus:outline-none"
					value={profile.ue4ss_control_mode}
					disabled={busy}
					onchange={(event) =>
						setOptions({ ue4ss_control_mode: event.currentTarget.value as Ue4ssControlMode })}
				>
					<option value="enabled_txt">{m.mods_order_mode_enabled_txt()}</option>
					<option value="mods_txt">{m.mods_order_mode_mods_txt()}</option>
				</select>
			</div>
		</section>
	</div>
{/if}
