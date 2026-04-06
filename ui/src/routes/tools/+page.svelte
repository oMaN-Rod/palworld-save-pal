<script lang="ts">
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { getAppState, getToastState } from '$states';
	import { Card } from '$components/ui';
	import { send, sendAndWait } from '$lib/utils/websocketUtils';
	import { MessageType, type GamepassSave } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { cn } from '$theme';
	import { GamepassBrowser } from '$components/gamepass';
	import { Spinner } from '$components/ui';
	import * as m from '$i18n/messages';
	import {
		ArrowRightLeft,
		Monitor,
		HardDrive,
		ArrowLeft,
		Gamepad2,
		RefreshCw,
		Hash,
		Copy,
		Check
	} from 'lucide-svelte';

	const appState = getAppState();
	const toast = getToastState();
	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	const steamIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/steam.svg`);
	const xboxIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/xbox.svg`);

	type Tab = 'convert' | 'gamepass' | 'steamid';
	let activeTab: Tab = $state('convert');

	// Convert tab state
	let isConverting = $state(false);
	let conversionResult = $state('');
	let convertGamepassSaves: Record<string, GamepassSave> = $state({});
	let showConvertBrowser = $state(false);
	let isConvertScanning = $state(false);

	// GamePass browser tab state
	let browserSaves: Record<string, GamepassSave> = $state({});
	let isBrowserScanning = $state(false);
	let browserLoaded = $state(false);

	// Steam ID tab state
	let steamInput = $state('');
	let steamConverting = $state(false);
	let steamResult: {
		palworld_uid?: string;
		nosteam_uid?: string;
		error?: string;
		from_uid?: boolean;
	} | null = $state(null);
	let copiedField: string | null = $state(null);

	const currentFormat = $derived(appState.saveFile?.type ?? null);
	const targetFormat = $derived(currentFormat === 'steam' ? 'gamepass' : 'steam');
	const hasLoadedSave = $derived(!!appState.saveFile);

	const tabs: { id: Tab; label: string; icon: typeof ArrowRightLeft }[] = [
		{ id: 'convert', label: 'Convert', icon: ArrowRightLeft },
		{ id: 'gamepass', label: 'GamePass Browser', icon: Gamepad2 },
		{ id: 'steamid', label: 'Steam ID', icon: Hash }
	];

	// --- Convert tab handlers ---

	function handleResult(result: { message?: string; error?: string }) {
		isConverting = false;
		if (result.error) {
			conversionResult = result.error;
			toast.add(result.error, 'Error', 'error');
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
			toast.add(`Conversion failed: ${err.message}`, 'Error', 'error');
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
			toast.add(`Conversion failed: ${err.message}`, 'Error', 'error');
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
				toast.add(result.error, 'Error', 'error');
			} else if (result.saves) {
				convertGamepassSaves = result.saves;
				showConvertBrowser = true;
			}
		} catch (err: any) {
			toast.add(`Failed to scan: ${err.message}`, 'Error', 'error');
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
			toast.add(`Conversion failed: ${err.message}`, 'Error', 'error');
		}
	}

	// --- GamePass browser tab handlers ---

	async function loadBrowserSaves() {
		isBrowserScanning = true;
		try {
			const result = await sendAndWait<{
				saves?: Record<string, GamepassSave>;
				error?: string;
			}>(MessageType.SCAN_GAMEPASS_SAVES, {});
			if (result.saves) {
				browserSaves = result.saves;
				browserLoaded = true;
			}
		} catch (err: any) {
			toast.add(`Failed to scan: ${err.message}`, 'Error', 'error');
		} finally {
			isBrowserScanning = false;
		}
	}

	function handleTabChange(tab: Tab) {
		activeTab = tab;
		if (tab === 'gamepass' && !browserLoaded && isDesktopMode) {
			loadBrowserSaves();
		}
	}

	// --- Steam ID tab handlers ---

	async function handleConvertSteamId() {
		if (!steamInput.trim()) return;
		steamConverting = true;
		steamResult = null;
		try {
			const result = await sendAndWait<{
				palworld_uid?: string;
				nosteam_uid?: string;
				error?: string;
			}>(MessageType.CONVERT_STEAM_ID, { steam_input: steamInput.trim() });
			steamResult = result;
			if (result.error) {
				toast.add(result.error, 'Error', 'error');
			}
		} catch (err: any) {
			steamResult = { error: err.message };
			toast.add(`Conversion failed: ${err.message}`, 'Error', 'error');
		} finally {
			steamConverting = false;
		}
	}

	async function copyToClipboard(text: string, field: string) {
		await navigator.clipboard.writeText(text);
		copiedField = field;
		setTimeout(() => (copiedField = null), 2000);
	}
</script>

<div class="flex min-h-screen w-full flex-col items-center py-8">
	<div class="flex w-full max-w-3xl flex-col gap-8">
		<!-- Header -->
		<section class="w-full">
			<h1 class="text-primary-400 mb-6 text-center text-4xl font-extrabold tracking-tight">
				{m.tools()}
			</h1>
		</section>

		<!-- Tabs -->
		<div class="border-surface-700 flex gap-1 border-b">
			{#each tabs as tab}
				<button
					class={cn(
						'flex items-center gap-2 px-4 py-2.5 text-sm font-medium transition-colors',
						activeTab === tab.id
							? 'text-primary-400 border-primary-400 border-b-2'
							: 'text-surface-400 hover:text-surface-200 border-b-2 border-transparent'
					)}
					onclick={() => handleTabChange(tab.id)}
				>
					<tab.icon size={16} />
					{tab.label}
				</button>
			{/each}
		</div>

		<!-- Convert Tab -->
		{#if activeTab === 'convert'}
			<div class="flex flex-col gap-8">
				{#if isConverting || isConvertScanning}
					<div class="flex flex-col items-center gap-4">
						<Spinner />
						{#if appState.progressMessage}
							<span class="text-surface-200">{appState.progressMessage}</span>
						{:else if isConvertScanning}
							<span class="text-surface-200">Scanning GamePass saves...</span>
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
							<h2 class="text-surface-100 text-2xl font-bold">
								GamePass → {m.steam()}
							</h2>
						</div>
						<p class="text-surface-400 mb-4 text-sm">
							Select a save to extract to Steam format
						</p>
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
											<span class="text-surface-300 text-sm capitalize"
												>{currentFormat}</span
											>
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
											<span class="text-surface-300 text-sm capitalize"
												>{targetFormat}</span
											>
										</div>
									</div>
									<p class="text-surface-300 text-center text-sm">
										Convert
										<strong>{appState.saveFile?.world_name ?? 'save'}</strong> from
										<span class="text-primary-400 capitalize">{currentFormat}</span>
										to
										<span class="text-primary-400 capitalize">{targetFormat}</span>
										format
									</p>
									<button
										class="btn bg-primary-500 hover:bg-primary-600 text-white"
										onclick={handleConvertLoaded}
									>
										<ArrowRightLeft size={16} />
										<span>Convert</span>
									</button>
								</div>
							</Card>
						</section>

						<hr class="border-surface-700" />
					{/if}

					<!-- Standalone Conversion -->
					{#if isDesktopMode}
						<section class="w-full">
							<h2 class="text-surface-100 mb-4 text-center text-2xl font-bold">
								{m.tools_convert_standalone()}
							</h2>
							<p class="text-surface-400 mb-6 text-center text-sm">
								Convert save files without loading them into the editor
							</p>
							<div
								class="grid w-full grid-cols-1 justify-center gap-8 sm:grid-cols-2"
							>
								<!-- Steam to GamePass -->
								<button
									type="button"
									class={cn(
										'bg-surface-800 hover:border-primary-500 border-surface-700 flex cursor-pointer flex-col items-center justify-between rounded-xl border-2 p-8 shadow-md transition-all'
									)}
									onclick={handleSteamToGamepass}
								>
									<div class="flex flex-col items-center gap-3">
										<div class="flex items-center gap-3">
											<div
												class="bg-surface-900 flex h-14 w-14 items-center justify-center rounded-full p-3"
											>
												{@html steamIcon}
											</div>
											<ArrowRightLeft class="text-surface-400" size={20} />
											<div
												class="bg-surface-900 flex h-14 w-14 items-center justify-center rounded-full p-3"
											>
												{@html xboxIcon}
											</div>
										</div>
										<span class="text-lg font-semibold text-white"
											>{m.steam()} → GamePass</span
										>
										<span class="text-surface-300 text-center text-sm">
											Import a Steam save directory into GamePass container format
										</span>
									</div>
								</button>

								<!-- GamePass to Steam -->
								<button
									type="button"
									class={cn(
										'bg-surface-800 hover:border-primary-500 border-surface-700 flex cursor-pointer flex-col items-center justify-between rounded-xl border-2 p-8 shadow-md transition-all'
									)}
									onclick={handleGamepassToSteamClick}
								>
									<div class="flex flex-col items-center gap-3">
										<div class="flex items-center gap-3">
											<div
												class="bg-surface-900 flex h-14 w-14 items-center justify-center rounded-full p-3"
											>
												{@html xboxIcon}
											</div>
											<ArrowRightLeft class="text-surface-400" size={20} />
											<div
												class="bg-surface-900 flex h-14 w-14 items-center justify-center rounded-full p-3"
											>
												{@html steamIcon}
											</div>
										</div>
										<span class="text-lg font-semibold text-white"
											>GamePass → {m.steam()}</span
										>
										<span class="text-surface-300 text-center text-sm">
											Extract GamePass containers to Steam save directory format
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
									<p class="text-surface-300 text-center">
										Save format conversion requires the desktop app for direct file
										system access.
									</p>
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
		{/if}

		<!-- Steam ID Tab -->
		{#if activeTab === 'steamid'}
			<div class="flex flex-col gap-8">
				<section class="w-full">
					<h2 class="text-surface-100 mb-2 text-center text-2xl font-bold">
						Steam ID Converter
					</h2>
					<p class="text-surface-400 mb-6 text-center text-sm">
						Convert a Steam ID to Palworld UID and NoSteam UID
					</p>

					<Card class="mx-auto max-w-lg">
						<div class="flex flex-col gap-4 p-4">
							<div class="flex flex-col gap-2">
								<label for="steam-input" class="text-surface-300 text-sm font-medium">
									Steam ID or Profile URL
								</label>
								<div class="flex gap-2">
									<input
										id="steam-input"
										type="text"
										bind:value={steamInput}
										placeholder="76561198012345678 or steamcommunity.com/profiles/..."
										class="bg-surface-800 border-surface-600 text-surface-100 placeholder:text-surface-500 flex-1 rounded-lg border px-3 py-2 text-sm focus:border-primary-500 focus:outline-none"
										onkeydown={(e) => e.key === 'Enter' && handleConvertSteamId()}
									/>
									<button
										class="btn bg-primary-500 hover:bg-primary-600 text-white"
										onclick={handleConvertSteamId}
										disabled={steamConverting || !steamInput.trim()}
									>
										{#if steamConverting}
											<Spinner />
										{:else}
											<Hash size={16} />
											<span>Convert</span>
										{/if}
									</button>
								</div>
								<span class="text-surface-500 text-xs">
									Accepts: numeric Steam ID, steam_ prefix, profile URL, or Palworld UID
								</span>
							</div>

							{#if steamResult && !steamResult.error}
								<div class="border-surface-700 flex flex-col gap-3 border-t pt-4">
									{#if steamResult.from_uid}
										<p class="text-surface-400 text-xs italic">
											Input detected as Palworld UID
										</p>
									{/if}
									<div class="flex flex-col gap-1">
										<span class="text-surface-400 text-xs font-medium uppercase tracking-wider">
											Palworld UID
										</span>
										<div class="flex items-center gap-2">
											<code class="bg-surface-900 text-primary-400 flex-1 rounded px-3 py-1.5 font-mono text-sm">
												{steamResult.palworld_uid}
											</code>
											<button
												class="text-surface-400 hover:text-surface-200 p-1"
												onclick={() =>
													copyToClipboard(steamResult!.palworld_uid!, 'palworld')}
											>
												{#if copiedField === 'palworld'}
													<Check size={16} class="text-green-400" />
												{:else}
													<Copy size={16} />
												{/if}
											</button>
										</div>
									</div>
									<div class="flex flex-col gap-1">
										<span class="text-surface-400 text-xs font-medium uppercase tracking-wider">
											NoSteam UID
										</span>
										<div class="flex items-center gap-2">
											<code class="bg-surface-900 text-primary-400 flex-1 rounded px-3 py-1.5 font-mono text-sm">
												{steamResult.nosteam_uid}
											</code>
											<button
												class="text-surface-400 hover:text-surface-200 p-1"
												onclick={() =>
													copyToClipboard(steamResult!.nosteam_uid!, 'nosteam')}
											>
												{#if copiedField === 'nosteam'}
													<Check size={16} class="text-green-400" />
												{:else}
													<Copy size={16} />
												{/if}
											</button>
										</div>
									</div>
								</div>
							{/if}

							{#if steamResult?.error}
								<div class="border-surface-700 border-t pt-4">
									<p class="text-red-400 text-sm">{steamResult.error}</p>
								</div>
							{/if}
						</div>
					</Card>
				</section>
			</div>
		{/if}

		<!-- GamePass Browser Tab -->
		{#if activeTab === 'gamepass'}
			<div class="flex flex-col gap-6">
				{#if !isDesktopMode}
					<Card class="mx-auto max-w-lg">
						<div class="flex flex-col items-center gap-4 p-4">
							<Monitor size={48} class="text-surface-400" />
							<p class="text-surface-300 text-center">
								GamePass browser requires the desktop app for direct file system access.
							</p>
						</div>
					</Card>
				{:else if isBrowserScanning}
					<div class="flex flex-col items-center gap-4">
						<Spinner />
						<span class="text-surface-200">Scanning GamePass saves...</span>
					</div>
				{:else}
					<div class="flex items-center justify-between">
						<p class="text-surface-400 text-sm">
							View and inspect your GamePass save files
						</p>
						<button
							class="text-surface-400 hover:text-surface-200 flex items-center gap-1.5 text-sm"
							onclick={loadBrowserSaves}
						>
							<RefreshCw size={14} />
							Refresh
						</button>
					</div>
					<GamepassBrowser
						bind:saves={browserSaves}
						manageable={true}
						onchange={loadBrowserSaves}
					/>
				{/if}
			</div>
		{/if}
	</div>
</div>
