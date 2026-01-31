<script lang="ts">
	import { Card, Tooltip } from '$components/ui';
	import { History, FolderDot } from '@lucide/svelte';
	import { onMount } from 'svelte';
	import { assetLoader, focusModal } from '$utils';
	import { send } from '$utils/websocketUtils';
	import * as m from '$i18n/messages';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { MessageType } from '$types';

	let {
		title = m.open_folder(),
		closeModal
	} = $props<{
		title?: string;
		closeModal: () => void;
	}>();

	type Folder = {
		name: string;
		icon: typeof History | string;
		folderType: string;
	};

	const steamIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/steam.svg`) as string;
	const xboxIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/xbox.svg`) as string;

	const folders: Folder[] = [
		{ name: 'Backups', icon: History, folderType: 'backups' },
		{ name: 'Steam', icon: steamIcon, folderType: 'steam' },
		{ name: 'Game pass', icon: xboxIcon, folderType: 'gamepass' },
		{ name: 'PSP Root', icon: FolderDot, folderType: 'psp_root' }
	];

	function handleFolderClick(folderType: string) {
		send(MessageType.OPEN_FOLDER, { folder_type: folderType });
		closeModal();
	}

	let modalContainer: HTMLDivElement;

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="min-w-auto">
		<h3 class="h3">{title}</h3>

		<div class="mt-2 flex gap-2">
			{#each folders as folder}
				<Tooltip>
					<button
						type="button"
						class="border-secondary-500/50 hover:bg-secondary-500/25 flex cursor-pointer flex-col items-center space-y-1 rounded-md border p-4"
						onclick={() => handleFolderClick(folder.folderType)}
					>
						{#if typeof folder.icon === 'string'}
							<div class="h-12 w-12">
								{@html folder.icon}
							</div>
						{:else}
							{@const Icon = folder.icon}
							<Icon class="h-12 w-12" />
						{/if}
					</button>
					{#snippet popup()}
						<span class="text-sm font-medium">{folder.name}</span>
					{/snippet}
				</Tooltip>
			{/each}
		</div>
	</Card>
</div>
