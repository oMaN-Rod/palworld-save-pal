<script lang="ts">
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { goto } from '$app/navigation';
	import { getAppState } from '$states';
	import { Card, Tooltip } from '$components/ui';
	import { getSocketState } from '$states';
	import { MessageType } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { cn } from '$theme';
	import { Download } from 'lucide-svelte';

	type SaveType = 'steam' | 'gamepass';

	const appState = getAppState();
	const ws = getSocketState();
	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	const steamIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/steam.svg`);
	const xboxIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/xbox.svg`);
	const morpheus = assetLoader.loadImage(`${ASSET_DATA_PATH}/img/app/morpheus.png`);

	function splitPath(path: string) {
		if (!path) {
			return {
				directory: '',
				filename: ''
			};
		}
		const normalizedPath = path.replace(/\\/g, '/');
		const lastSlashIndex = normalizedPath.lastIndexOf('/');

		if (lastSlashIndex === -1) {
			return {
				directory: '',
				filename: normalizedPath
			};
		}

		return {
			directory: normalizedPath.slice(0, lastSlashIndex),
			filename: normalizedPath.slice(lastSlashIndex + 1)
		};
	}

	async function handleSelectSave(saveType: SaveType) {
		await goto('/loading');
		ws.send(
			JSON.stringify({
				type: MessageType.SELECT_SAVE,
				data: {
					type: saveType,
					local: isDesktopMode
				}
			})
		);
	}

	async function handleSave() {
		await goto('/loading');
		ws.send(
			JSON.stringify({
				type: MessageType.SAVE_MODDED_SAVE
			})
		);
	}

	$effect(() => {
		if (!isDesktopMode) {
			goto('/upload');
		}
	});
</script>

{#snippet pickYourPoison(size: 'sm' | 'lg' = 'lg')}
	{@const morpheusSize = size === 'lg' ? 'h-[600px]' : 'h-[400px]'}
	{@const inset = size === 'lg' ? 'inset-8' : 'inset-4'}
	{@const offset = size === 'lg' ? 'top-[420px]' : 'top-[280px]'}
	{@const pillSize = size === 'lg' ? 'h-24 w-24' : 'h-16 w-16'}
	<div class="relative flex flex-col">
		<div class="relative">
			<img src={morpheus} alt="Choice" class={cn('w-auto', morpheusSize)} />
			<div class={cn('absolute flex items-center justify-between px-12', inset, offset)}>
				<Tooltip
					popupClass="p-0 bg-surface-600 max-w-96"
					rounded="rounded-none"
					position="left"
					useArrow={false}
				>
					<button
						class={cn('rounded-full bg-transparent transition-transform hover:scale-110', pillSize)}
						onclick={() => handleSelectSave('steam')}
					>
						{#if steamIcon}
							{@html steamIcon}
						{:else}
							Steam
						{/if}
					</button>
					{#snippet popup()}
						<div class="flex flex-col p-4">
							<h4 class="h4">Steam</h4>
							<p>Find and select your Level.sav file.</p>
						</div>
					{/snippet}
				</Tooltip>

				<Tooltip
					popupClass="p-0 bg-surface-600 max-w-96"
					rounded="rounded-none"
					position="right"
					useArrow={false}
				>
					<button
						class={cn(
							'-rotate-90 rounded-full bg-transparent transition-transform hover:scale-110',
							pillSize
						)}
						onclick={() => handleSelectSave('gamepass')}
						disabled
					>
						{#if xboxIcon}
							{@html xboxIcon}
						{:else}
							Xbox
						{/if}
					</button>
					{#snippet popup()}
						<div class="flex flex-col p-4">
							<h4 class="h4">XBOX Game Pass</h4>
							<p>Coming soon!™</p>
						</div>
					{/snippet}
				</Tooltip>
			</div>
		</div>
	</div>
{/snippet}

<div class="flex h-full w-full flex-col items-center justify-center space-y-4">
	{#if appState.saveFile && appState.playerSaveFiles}
		{@const { directory, filename } = splitPath(appState.saveFile.name)}
		{@render pickYourPoison('sm')}
		<Card class="min-w-96">
			<div class="flex flex-col">
				<h4 class="h4">Loaded Files</h4>
				<p class="text"><strong>Path:</strong> {directory}</p>
				<p class="text"><strong>Level:</strong> {filename}</p>
				<p class="text"><strong>World Name:</strong> {appState.saveFile.world_name}</p>
				<p class="text"><strong>Players ({appState.playerSaveFiles.length}):</strong></p>
				<ul class="max-h-36 list-inside list-disc overflow-y-scroll">
					{#each appState.playerSaveFiles as playerSaveFile}
						<li>{playerSaveFile.name.replace(/-/g, '').toUpperCase()}.sav</li>
					{/each}
				</ul>
			</div>
		</Card>
		<div class="flex space-x-4">
			<Tooltip>
				<button class="btn preset-filled-primary-500 font-bold" onclick={handleSave}>
					<Download /> Save
				</button>
				{#snippet popup()}
					<span>Save modified Level.sav file</span>
				{/snippet}
			</Tooltip>
		</div>
	{:else}
		<div class="relative flex h-full w-full items-center justify-center">
			{@render pickYourPoison()}
		</div>
	{/if}
</div>
