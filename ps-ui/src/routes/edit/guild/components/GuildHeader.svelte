<script lang="ts">
	import { Button, Nuke, Tooltip } from '$components/ui';
	import { DebugButton } from '$components/layout';
	import type { Base, Guild } from '$types';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	interface Props {
		guild: Guild;
		base: Base | null;
		baseNumber: number;
		debugMode?: boolean;
		onEditGuildName: () => void;
		onEditBasecampLevel: () => void;
		onDeleteGuild: () => void;
		onEditBaseName: () => void;
	}

	let {
		guild,
		base,
		baseNumber,
		debugMode = false,
		onEditGuildName,
		onEditBasecampLevel,
		onDeleteGuild,
		onEditBaseName
	}: Props = $props();
</script>

<div class="flex">
	<div class="flex items-center">
		<Button
			id="guild-name"
			variant="ghost"
			class="min-w-0 px-0 text-start"
			onclick={onEditGuildName}
		>
			<h4 class="h4 hover:text-secondary-500 truncate">{guild.name}</h4>
		</Button>
		<Tooltip label={m.basecamp_level()}>
			<button
				id="guild-level"
				class="outline-surface-700 hover:outline-secondary-500 ml-2 flex gap-2 rounded p-1 align-bottom outline"
				onclick={onEditBasecampLevel}
			>
				<span class="text-surface-700">{m.level_abbr()}</span>
				{guild.base_camp_level}
			</button>
		</Tooltip>
	</div>
	{#if debugMode}
		<DebugButton href={`/debug?guildId=${guild.id}`} />
	{/if}
	<Tooltip label={m.delete_entire_guild()}>
		<button
			id="guild-delete"
			class="btn ml-4 h-8 w-8 p-2 hover:bg-red-500/50"
			onclick={onDeleteGuild}
		>
			<Nuke size={24} />
		</button>
	</Tooltip>
</div>

<div class="flex flex-col">
	<div class="flex">
		<h5 class="h5 font-light">{c.base} {baseNumber}</h5>
		{#if base && debugMode}
			<DebugButton iconClass="h-4 w-4" href={`/debug?guildId=${guild.id}&baseId=${base.id}`} />
		{/if}
	</div>
	<div class="flex">
		<Button id="guild-base-name" variant="ghost" class="px-0" onclick={onEditBaseName}>
			<h5 class="h5 hover:text-secondary-500 font-light">
				{base?.name || ''}
			</h5>
		</Button>
	</div>
</div>
