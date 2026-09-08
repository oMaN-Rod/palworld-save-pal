<script lang="ts">
	import { lastOnlineDate } from './liveGuild.utils';
	import { cn } from '$theme';
	import type { GameGuildMemberJson } from '$states/gameState.svelte';
	import * as m from '$i18n/messages';

	let {
		members = [],
		memberUids = [],
		adminUid = null,
		roleOptions = [],
		roleBusy = false,
		setRoleReason,
		onSetMemberRole
	}: {
		members?: GameGuildMemberJson[];
		memberUids?: string[];
		adminUid?: string | null;
		roleOptions?: string[];
		roleBusy?: boolean;
		setRoleReason?: string;
		onSetMemberRole?: (memberUid: string, role: string) => void;
	} = $props();

	const relative = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });

	function lastOnline(ticks: number | null): string {
		const date = lastOnlineDate(ticks);
		if (!date) return '—';
		const days = Math.round((date.getTime() - Date.now()) / 86_400_000);
		return relative.format(days, 'day');
	}
</script>

{#if members.length > 0}
	<ul id="live-guild-members" class="divide-surface-800 border-surface-800 divide-y rounded-sm border">
		{#each members as member, index (member.uid ?? `row-${index}`)}
			<li class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 p-2 @md/guild:grid-cols-[minmax(0,1fr)_8rem_5rem_6rem]">
				<div class="flex min-w-0 flex-col">
					<span class="truncate text-sm font-medium">{member.name || member.uid}</span>
					{#if member.uid && member.uid === adminUid}
						<span class="text-primary-400 text-xs">{m.guild_leader()}</span>
					{/if}
				</div>

				{#if member.uid && roleOptions.length > 0}
					<label class="sr-only" for={`live-guild-role-${member.uid}`}>
						{m.live_guild_role_for({ name: member.name || member.uid })}
					</label>
					<select
						id={`live-guild-role-${member.uid}`}
						class="select w-32 text-xs"
						value={member.role ?? ''}
						disabled={roleBusy || !!setRoleReason}
						onchange={(event: Event) =>
							onSetMemberRole?.(member.uid as string, (event.currentTarget as HTMLSelectElement).value)}
					>
						{#each roleOptions as role (role)}
							<option value={role}>{role}</option>
						{/each}
					</select>
				{:else if member.role}
					<span class="text-surface-400 text-xs">{member.role}</span>
				{:else}
					<span></span>
				{/if}

				<span
					class={cn(
						'rounded-full px-2 text-xs @max-md/guild:hidden',
						member.status === 'Online' ? 'text-success-400' : 'text-surface-400'
					)}
				>
					{member.status ?? '—'}
				</span>

				<span class="text-surface-400 text-xs tabular-nums @max-md/guild:hidden">
					<span class="sr-only">{m.live_guild_last_online()}: </span>{lastOnline(member.lastOnlineTicks)}
				</span>
			</li>
		{/each}
	</ul>
{:else if memberUids.length > 0}
	<ul id="live-guild-members" class="flex flex-col gap-1">
		{#each memberUids as uid (uid)}
			<li class="font-mono text-xs break-all">{uid}</li>
		{/each}
	</ul>
	<p class="text-surface-400 mt-2 text-xs">{m.live_guild_members_uid_only()}</p>
{:else}
	<span class="text-surface-400 text-sm">—</span>
{/if}
