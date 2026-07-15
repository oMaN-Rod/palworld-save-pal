<script lang="ts">
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { getAppState, getToastState } from '$states';
	import { Button, Card, Spinner } from '$components/ui';
	import { sendAndWait } from '$lib/utils/websocketUtils';
	import { MessageType, type GamepassSave } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { GamepassBrowser } from '$components/gamepass';
	import { c } from '$lib/utils/commonTranslations';
	import * as m from '$i18n/messages';
	import { ArrowRightLeft, Monitor, HardDrive, ArrowLeft } from 'lucide-svelte';

	const appState = getAppState();
	const toast = getToastState();
	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	const steamIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/steam.svg`);
	const xboxIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/xbox.svg`);

	let isConverting = $state(false);
	let conversionResult = $state('');
	let convertGamepassSaves: Record<string, GamepassSave> = $state({});
	let showConvertBrowser = $state(false);
	let isConvertScanning = $state(false);

	const currentFormat = $derived(appState.saveFile?.type ?? null);
	const targetFormat = $derived(currentFormat === 'steam' ? 'gamepass' : 'steam');
	const hasLoadedSave = $derived(!!appState.saveFile);

	// 'steam' | 'gamepass' -> its display label.
	function formatLabel(format: string | null): string {
		return format === 'steam' ? m.steam() : m.gamepass();
	}

	function handleResult(result: { message?: string; error?: string }) {
		isConverting = false;
		if (result.error) {
			conversionResult = result.error;
			toast.add(result.error, m.error(), 'error');
		} else if (result.message) {
			conversionResult = result.message;
			toast.add(result.message);
		}
	}

	async function handleConvertLoaded() {
		if (!hasLoadedSave || !isDesktopMode) return;
		isConverting = true;
		conversionResult = '';
		try {
			const result = await sendAndWait<{ message?: string; error?: string }>(
				MessageType.CONVERT_SAVE_FORMAT,
				{ target_format: targetFormat }
			);
			handleResult(result);
		} catch (err: any) {
			isConverting = false;
			conversionResult = `Error: ${err.message}`;
			toast.add(m.tools_conversion_failed({ error: err.message }), m.error(), 'error');
		}
	}

	async function handleSteamToGamepass() {
		if (!isDesktopMode) return;
		isConverting = true;
		conversionResult = '';
		try {
			const result = await sendAndWait<{ message?: string; error?: string }>(
				MessageType.CONVERT_SAVE_FORMAT,
				{ target_format: 'gamepass', source_path: '__select__', output_path: '__select__' }
			);
			handleResult(result);
		} catch (err: any) {
			isConverting = false;
			conversionResult = `Error: ${err.message}`;
			toast.add(m.tools_conversion_failed({ error: err.message }), m.error(), 'error');
		}
	}

	async function handleGamepassToSteamClick() {
		if (!isDesktopMode) return;
		isConvertScanning = true;
		try {
			const result = await sendAndWait<{
				saves?: Record<string, GamepassSave>;
				error?: string;
			}>(MessageType.SCAN_GAMEPASS_SAVES, {});
			if (result.error) {
				toast.add(result.error, m.error(), 'error');
			} else if (result.saves) {
				convertGamepassSaves = result.saves;
				showConvertBrowser = true;
			}
		} catch (err: any) {
			toast.add(m.tools_scan_failed({ error: err.message }), m.error(), 'error');
		} finally {
			isConvertScanning = false;
		}
	}

	async function handleGamepassSaveSelected(save: GamepassSave) {
		showConvertBrowser = false;
		isConverting = true;
		conversionResult = '';
		try {
			const result = await sendAndWait<{ message?: string; error?: string }>(
				MessageType.CONVERT_SAVE_FORMAT,
				{ target_format: 'steam', save_id: save.save_id }
			);
			handleResult(result);
		} catch (err: any) {
			isConverting = false;
			conversionResult = `Error: ${err.message}`;
			toast.add(m.tools_conversion_failed({ error: err.message }), m.error(), 'error');
		}
	}
</script>

<div class="flex flex-col gap-8">
	{#if isConverting || isConvertScanning}
		<div class="flex flex-col items-center gap-4">
			<Spinner />
			{#if appState.progressMessage}
				<span class="text-surface-200">{appState.progressMessage}</span>
			{:else if isConvertScanning}
				<span class="text-surface-200">{m.tools_scanning_gamepass()}</span>
			{/if}
		</div>
	{:else if showConvertBrowser}
		<section class="w-full">
			<div class="mb-4 flex items-center gap-3">
				<button
					class="text-surface-400 hover:text-surface-200"
					onclick={() => (showConvertBrowser = false)}
				>
					<ArrowLeft size={20} />
				</button>
				<h2 class="text-surface-100 text-2xl font-bold">{m.gamepass()} → {m.steam()}</h2>
			</div>
			<p class="text-surface-400 mb-4 text-sm">{m.tools_extract_to_steam_hint()}</p>
			<GamepassBrowser
				saves={convertGamepassSaves}
				selectable={true}
				onselect={handleGamepassSaveSelected}
			/>
		</section>
	{:else}
		<!-- Convert Loaded Save -->
		{#if hasLoadedSave && isDesktopMode}
			<section class="w-full">
				<h2 class="text-surface-100 mb-4 text-center text-2xl font-bold">
					{m.tools_convert_loaded()}
				</h2>
				<Card class="mx-auto max-w-lg">
					<div class="flex flex-col items-center gap-4 p-4">
						<div class="flex items-center gap-4">
							<div class="flex flex-col items-center gap-1">
								<div
									class="bg-surface-800 flex h-16 w-16 items-center justify-center rounded-full p-3"
								>
									{#if currentFormat === 'steam'}
										{@html steamIcon}
									{:else}
										{@html xboxIcon}
									{/if}
								</div>
								<span class="text-surface-300 text-sm">{formatLabel(currentFormat)}</span>
							</div>
							<ArrowRightLeft class="text-primary-400" size={28} />
							<div class="flex flex-col items-center gap-1">
								<div
									class="bg-surface-800 flex h-16 w-16 items-center justify-center rounded-full p-3"
								>
									{#if targetFormat === 'steam'}
										{@html steamIcon}
									{:else}
										{@html xboxIcon}
									{/if}
								</div>
								<span class="text-surface-300 text-sm">{formatLabel(targetFormat)}</span>
							</div>
						</div>
						<p class="text-surface-300 text-center text-sm">
							{m.tools_convert_loaded_prompt({
								world: appState.saveFile?.world_name ?? c.save,
								source: formatLabel(currentFormat),
								target: formatLabel(targetFormat)
							})}
						</p>
						<Button variant="primary" onclick={handleConvertLoaded}>
							<ArrowRightLeft size={16} />
							<span>{m.tools_convert_action()}</span>
						</Button>
					</div>
				</Card>
			</section>

			<hr class="border-surface-700" />
		{/if}

		<!-- Standalone Conversion -->
		{#if isDesktopMode}
			<section class="w-full">
				<p class="text-surface-400 mb-6 text-center text-sm">{m.tools_convert_standalone_desc()}</p>
				<div class="grid w-full grid-cols-1 justify-center gap-8 sm:grid-cols-2">
					<!-- Steam to GamePass -->
					<button
						type="button"
						class="bg-surface-800 hover:border-primary-500 border-surface-700 flex cursor-pointer flex-col items-center justify-between rounded-xl border-2 p-8 shadow-md transition-all"
						onclick={handleSteamToGamepass}
					>
						<div class="flex flex-col items-center gap-3">
							<div class="flex items-center gap-3">
								<div class="bg-surface-900 flex h-14 w-14 items-center justify-center rounded-full p-3">
									{@html steamIcon}
								</div>
								<ArrowRightLeft class="text-surface-400" size={20} />
								<div class="bg-surface-900 flex h-14 w-14 items-center justify-center rounded-full p-3">
									{@html xboxIcon}
								</div>
							</div>
							<span class="text-surface-50 text-lg font-semibold">{m.steam()} → {m.gamepass()}</span>
							<span class="text-surface-300 text-center text-sm">
								{m.tools_steam_to_gamepass_desc()}
							</span>
						</div>
					</button>

					<!-- GamePass to Steam -->
					<button
						type="button"
						class="bg-surface-800 hover:border-primary-500 border-surface-700 flex cursor-pointer flex-col items-center justify-between rounded-xl border-2 p-8 shadow-md transition-all"
						onclick={handleGamepassToSteamClick}
					>
						<div class="flex flex-col items-center gap-3">
							<div class="flex items-center gap-3">
								<div class="bg-surface-900 flex h-14 w-14 items-center justify-center rounded-full p-3">
									{@html xboxIcon}
								</div>
								<ArrowRightLeft class="text-surface-400" size={20} />
								<div class="bg-surface-900 flex h-14 w-14 items-center justify-center rounded-full p-3">
									{@html steamIcon}
								</div>
							</div>
							<span class="text-surface-50 text-lg font-semibold">{m.gamepass()} → {m.steam()}</span>
							<span class="text-surface-300 text-center text-sm">
								{m.tools_gamepass_to_steam_desc()}
							</span>
						</div>
					</button>
				</div>
			</section>
		{:else}
			<section class="w-full">
				<Card class="mx-auto max-w-lg">
					<div class="flex flex-col items-center gap-4 p-4">
						<Monitor size={48} class="text-surface-400" />
						<p class="text-surface-300 text-center">{m.tools_convert_desktop_required()}</p>
					</div>
				</Card>
			</section>
		{/if}

		<!-- Conversion Result -->
		{#if conversionResult}
			<Card class="mx-auto max-w-lg">
				<div class="flex items-center gap-3 p-4">
					<HardDrive size={20} class="text-primary-400" />
					<span class="text-surface-200">{conversionResult}</span>
				</div>
			</Card>
		{/if}
	{/if}
</div>
