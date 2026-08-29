<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { List } from '$components/ui';
	import { send } from '$lib/utils/websocketUtils';
	import { MessageType, type GamepassSave } from '$types';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let { saves = $bindable() } = $props<{
		saves: Record<string, GamepassSave>;
	}>();

	let gamepassSaves: GamepassSave[] = $state([]);

	async function handleSelectSave(save: GamepassSave) {
		console.log('handleSelectSave', save);
		send(MessageType.SELECT_GAMEPASS_SAVE, save.save_id);
	}

	$effect(() => {
		gamepassSaves = Object.values(saves);
	});
</script>

<div class="flex flex-col space-y-4">
	<h2 class="h2">{m.available_entity({ entity: c.saves })}</h2>
	{#if saves && Object.keys(saves).length > 0}
		<List
			baseClass="bg-surface-800 max-h-64 2xl:max-h-[500px]"
			bind:items={gamepassSaves}
			idKey="save_id"
			canSelect={false}
			headerClass=""
			onselect={handleSelectSave}
		>
			{#snippet listHeader()}
				<div class="flex w-full">
					<span class="grow font-bold">{m.world_name()}</span>
					<span class="font-bold">{c.players}</span>
				</div>
			{/snippet}
			{#snippet listItem(save: GamepassSave)}
				<div class="flex w-full items-center">
					<span class="grow">{save.world_name}</span>
					<div class="flex items-center space-x-2">
						<Icon icon="tabler:users" size={16} />
						<span>{save.player_count}</span>
					</div>
				</div>
			{/snippet}
			{#snippet listItemPopup(save)}
				<div class="flex flex-col">
					<span class="font-bold">{save.world_name}</span>
					<span>Save ID: {save.save_id}</span>
					<span>Players: {save.player_count}</span>
				</div>
			{/snippet}
		</List>
	{/if}
</div>
