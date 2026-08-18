<script lang="ts">
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { goto } from '$app/navigation';
	import { getAppState, getModalState, getOverviewState, overviewViewMode } from '$states';
	import { Button, Spinner } from '$components/ui';
	import { GamepassBrowser } from '$components/gamepass';
	import { TextInputModal } from '$components/modals';
	import { openWorldOptionModal } from '$components/worldoption';
	import { send, pushProgressMessage } from '$lib/utils/websocketUtils';
	import { MessageType } from '$types';
	import type { GamepassSave } from '$types';
	import { untrack } from 'svelte';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { cn } from '$theme';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import Box from '@lucide/svelte/icons/box';
	import Building2 from '@lucide/svelte/icons/building-2';
	import Download from '@lucide/svelte/icons/download';
	import FileJson from '@lucide/svelte/icons/file-json';
	import Gem from '@lucide/svelte/icons/gem';
	import MapPin from '@lucide/svelte/icons/map-pin';
	import RefreshCw from '@lucide/svelte/icons/refresh-cw';
	import Settings2 from '@lucide/svelte/icons/settings-2';
	import ShieldCheck from '@lucide/svelte/icons/shield-check';
	import Sparkles from '@lucide/svelte/icons/sparkles';
	import Users from '@lucide/svelte/icons/users';
	import OverviewTile from './components/OverviewTile.svelte';
	import NeedsReviewCard from './components/NeedsReviewCard.svelte';
	import TraitsCard from './components/TraitsCard.svelte';
	import CompositionCard from './components/CompositionCard.svelte';
	import FunCard from './components/FunCard.svelte';
	import TopSpeciesCard from './components/TopSpeciesCard.svelte';
	import TopPlayersCard from './components/TopPlayersCard.svelte';

	const appState = getAppState();
	const modal = getModalState();
	const overviewState = getOverviewState();
	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	const steamIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/steam.svg`);
	const xboxIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/xbox.svg`);

	const viewMode = overviewViewMode;

	function formatBytes(bytes?: number): string {
		if (!bytes) return '—';
		const units = ['B', 'KB', 'MB', 'GB'];
		const index = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
		return `${(bytes / 1024 ** index).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
	}

	async function handleSelectSave(saveType: string) {
		await goto('/loading');
		send(MessageType.SELECT_SAVE, {
			type: saveType,
			local: isDesktopMode
		});
	}

	async function handleSelectGamepassSave(save: GamepassSave) {
		await goto('/loading');
		send(MessageType.SELECT_GAMEPASS_SAVE, save.save_id);
	}

	async function handleEditWorldName() {
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: m.edit_entity({ entity: m.world_name() }),
			value: appState.saveFile!.world_name
		});
		if (!result) return;
		appState.saveFile!.world_name = result;
		send(MessageType.RENAME_WORLD, result);
	}

	function exportJson() {
		if (!overviewState.stats) return;
		const world = appState.saveFile?.world_name ?? 'world';
		const stamp = new Date().toISOString().replace(/[:.]/g, '-');
		const blob = new Blob([JSON.stringify(overviewState.stats, null, 2)], {
			type: 'application/json'
		});
		const url = URL.createObjectURL(blob);
		const anchor = document.createElement('a');
		anchor.href = url;
		anchor.download = `overview_${world}_${stamp}.json`;
		anchor.click();
		URL.revokeObjectURL(url);
	}

	async function handleDownloadSaveFile() {
		send(MessageType.DOWNLOAD_SAVE_FILE);
		await goto('/loading');
		pushProgressMessage(m.upload_starting_to_cook());
	}

	// The stats are computed on demand and cached per session; fetch whenever a
	// save is present so the minimal view's tiles carry real numbers (the
	// expanded dashboard renders instantly when toggled). Drop the cache once
	// the save goes away (eject / session loss). untrack keeps the effect
	// keyed on the saveFile reference alone — the stats writes inside load()
	// must not re-trigger it, or every response refetches in a loop.
	$effect(() => {
		if (appState.saveFile) {
			untrack(() => overviewState.load());
		} else if (overviewState.stats || overviewState.error) {
			overviewState.reset();
		}
	});

	// Same sub-tab treatment as the breeding/tools pages so the toggle reads as
	// one control family across the app.
	function tabPill(active: boolean) {
		return cn(
			'rounded-sm px-3.5 py-1.5 text-xs font-medium transition-all',
			active
				? 'bg-surface-800 text-surface-50 border-surface-600/60 border shadow-sm'
				: 'text-surface-400 hover:bg-surface-800/60 hover:text-surface-200 border border-transparent'
		);
	}

	const saveOptions = $derived([
		{
			type: 'steam',
			title: m.steam(),
			icon: steamIcon,
			description: m.steam_description(),
			disabled: false
		},
		{
			type: 'gamepass',
			title: m.xbox_game_pass(),
			icon: xboxIcon,
			description: m.xbox_game_pass_description(),
			disabled: false
		}
	]);

	const stats = $derived(overviewState.stats);
</script>

<div class="animate-fade-in min-h-screen w-full">
	<div class="mx-auto flex w-full max-w-6xl flex-col gap-6 px-4 py-6 sm:px-6 lg:py-8">
		{#if appState.saveFile}
			<!-- ── Loaded save: the overview dashboard ── -->
			<header class="glass-panel flex flex-wrap items-center justify-between gap-4 p-4">
				<div class="flex min-w-0 flex-col items-start gap-1">
					<h1 class="heading-gradient text-2xl font-extrabold tracking-tight sm:text-3xl">
						{m.overview()}
					</h1>
					<button
						type="button"
						class="text-surface-100 hover:text-secondary-400 max-w-full truncate text-left text-sm font-semibold transition-colors"
						onclick={handleEditWorldName}
						title={m.edit_entity({ entity: m.world_name() })}
					>
						{appState.saveFile.world_name}
					</button>
					<span class="text-surface-500 text-xs">
						{appState.saveFile.type === 'gamepass' ? m.xbox_game_pass() : m.steam()}
						· {formatBytes(appState.saveFile.size)}
					</span>
				</div>
				<div class="flex flex-col items-end gap-2">
					<!-- Action row, top right -->
					<div class="flex flex-wrap items-center justify-end gap-2">
						{#if appState.saveFile.world_option_present}
							<Button variant="outline" size="sm" onclick={openWorldOptionModal}>
								<Settings2 size={14} />
								{m.overview_edit_world_options()}
							</Button>
						{/if}
						<Button variant="outline" size="sm" onclick={handleDownloadSaveFile}>
							<Download size={14} />
							{m.download()}
						</Button>
						<Button variant="outline" size="sm" onclick={exportJson} disabled={!stats}>
							<FileJson size={14} />
							{m.overview_export_json()}
						</Button>
						<Button
							variant="outline"
							size="sm"
							onclick={() => overviewState.load(true)}
							disabled={overviewState.loading}
						>
							<RefreshCw size={14} class={overviewState.loading ? 'animate-spin' : ''} />
							{m.overview_refresh()}
						</Button>
					</div>
					<!-- View-mode slider, beneath the actions -->
					<div
						class="border-surface-700/60 bg-surface-900/60 flex items-center gap-0.5 rounded-md border p-1"
						role="group"
						aria-label={m.overview()}
					>
						<button
							type="button"
							class={tabPill(viewMode.current === 'minimal')}
							onclick={() => (viewMode.current = 'minimal')}
						>
							{m.overview_view_minimal()}
						</button>
						<button
							type="button"
							class={tabPill(viewMode.current === 'expanded')}
							onclick={() => (viewMode.current = 'expanded')}
						>
							{m.overview_view_expanded()}
						</button>
					</div>
				</div>
			</header>

			{#if viewMode.current === 'minimal'}
				<!-- ── Minimal view: summary tiles + highlights, PalSavTools-style ── -->
				{#if overviewState.loading && !stats}
					<div class="flex h-64 items-center justify-center">
						<Spinner size="size-12" />
					</div>
				{:else if overviewState.error && !stats}
					<div
						class="border-error-500/40 bg-error-500/10 text-error-300 rounded-md border p-4 text-sm"
					>
						{overviewState.error}
					</div>
				{:else if stats}
					<section aria-label={m.overview_world_summary()}>
						<div class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5">
							<OverviewTile label={c.guilds} value={stats.totals.guilds} icon={Building2} />
							<OverviewTile label={c.players} value={stats.totals.players} icon={Users} />
							<OverviewTile label={c.bases} value={stats.totals.bases} icon={MapPin} />
							<OverviewTile
								label={m.overview_containers()}
								value={stats.totals.containers}
								icon={Box}
							/>
							<OverviewTile
								label={c.pals}
								value={stats.totals.pals}
								icon={Sparkles}
								accent="text-secondary-400"
							/>
						</div>
					</section>

					<!-- Highlights: traits & conditions at a glance -->
					<section aria-label={m.overview_traits()} class="flex flex-col gap-3">
						<h2 class="text-surface-400 text-xs font-semibold tracking-wider uppercase">
							{m.overview_traits()} & {m.overview_conditions()}
						</h2>
						<TraitsCard traits={stats.traits} condition={stats.condition} />
					</section>

					<FunCard {stats} />

					<!-- Top species & players -->
					<div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
						<TopSpeciesCard species={stats.top_species} />
						<TopPlayersCard players={stats.top_players} />
					</div>
				{:else}
					<div class="flex h-64 items-center justify-center">
						<Spinner size="size-12" />
					</div>
				{/if}
			{:else if overviewState.loading && !stats}
				<div class="flex h-64 items-center justify-center">
					<Spinner size="size-12" />
				</div>
			{:else if overviewState.error && !stats}
				<div
					class="border-error-500/40 bg-error-500/10 text-error-300 rounded-md border p-4 text-sm"
				>
					{overviewState.error}
				</div>
			{:else if stats}
				<!-- ── Expanded view: the full dashboard ── -->

				<!-- World summary + extended totals -->
				<section aria-label={m.overview_world_summary()} class="flex flex-col gap-3">
					<h2 class="text-surface-400 text-xs font-semibold tracking-wider uppercase">
						{m.overview_world_summary()}
					</h2>
					<div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
						<OverviewTile label={c.guilds} value={stats.totals.guilds} icon={Building2} />
						<OverviewTile label={c.players} value={stats.totals.players} icon={Users} />
						<OverviewTile label={c.bases} value={stats.totals.bases} icon={MapPin} />
						<OverviewTile
							label={m.overview_containers()}
							value={stats.totals.containers}
							icon={Box}
						/>
						<OverviewTile
							label={c.pals}
							value={stats.totals.pals}
							icon={Sparkles}
							accent="text-secondary-400"
						/>
						<OverviewTile
							label={m.overview_creature_pals()}
							value={stats.totals.creature_pals}
							icon={Sparkles}
						/>
						<OverviewTile
							label={m.overview_human_npcs()}
							value={stats.totals.human_npcs}
							icon={Users}
							accent="text-tertiary-400"
						/>
						<OverviewTile
							label={m.overview_species()}
							value={stats.totals.species}
							icon={Gem}
							accent="text-tertiary-400"
						/>
					</div>
				</section>

				<!-- Pals needing review -->
				{#if stats.anomalies.pal_count > 0}
					<NeedsReviewCard anomalies={stats.anomalies} />
				{/if}

				<!-- Traits & conditions -->
				<section aria-label={m.overview_traits()} class="flex flex-col gap-3">
					<h2 class="text-surface-400 text-xs font-semibold tracking-wider uppercase">
						{m.overview_traits()} & {m.overview_conditions()}
					</h2>
					<TraitsCard traits={stats.traits} condition={stats.condition} />
				</section>

				<!-- Pal composition -->
				<CompositionCard composition={stats.composition} />

				<FunCard {stats} />
				<!-- Top species & players -->
				<div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
					<TopSpeciesCard species={stats.top_species} />
					<TopPlayersCard players={stats.top_players} />
				</div>

				{#if stats.anomalies.pal_count === 0}
					<div
						class="border-success-500/40 bg-success-500/10 text-success-300 flex items-center justify-center gap-2 rounded-md border px-4 py-4 text-sm font-medium"
					>
						<ShieldCheck size={18} />
						{m.overview_no_flagged()}
					</div>
				{/if}
			{:else}
				<div class="flex h-64 items-center justify-center">
					<Spinner size="size-12" />
				</div>
			{/if}
		{:else}
			<!-- ── No save loaded: the save picker (desktop). On the web build the
				 save-required shell routes to /upload, so this branch only ever
				 flashes here during the bootstrap race; the spinner below covers it. -->
			{#if isDesktopMode}
				<section class="w-full">
					<h1 class="heading-gradient mb-6 text-center text-4xl font-extrabold tracking-tight">
						{m.select_entity({ entity: m.save_platform() })}
					</h1>
					<div
						class="mx-auto grid w-full max-w-3xl grid-cols-1 justify-center gap-8 sm:grid-cols-2"
					>
						{#each saveOptions as option (option.type)}
							<button
								type="button"
								class={cn(
									'group card-hover bg-surface-800/70 dark:bg-surface-800/70 flex flex-col items-center justify-between rounded-md border-2 p-8 shadow-md backdrop-blur-md',
									option.disabled
										? 'border-surface-700 cursor-not-allowed opacity-50'
										: 'border-surface-700/60 cursor-pointer'
								)}
								onclick={() => !option.disabled && handleSelectSave(option.type)}
								disabled={option.disabled}
							>
								<div class="flex flex-col items-center gap-2">
									<div
										class="bg-surface-900/60 group-hover:shadow-glow-paldium mb-2 flex h-24 w-24 items-center justify-center rounded-full p-4 shadow transition-all duration-200"
									>
										{@html option.icon}
									</div>
									<span class="text-surface-50 text-xl font-semibold">{option.title}</span>
									<span class="text-surface-400 text-center text-base">{option.description}</span>
								</div>
							</button>
						{/each}
					</div>
				</section>

				{#if appState.gamepassSaves && Object.keys(appState.gamepassSaves).length > 0}
					<GamepassBrowser
						saves={appState.gamepassSaves}
						selectable={true}
						onselect={handleSelectGamepassSave}
					/>
				{/if}
			{:else}
				<div class="flex h-64 items-center justify-center">
					<Spinner size="size-12" />
				</div>
			{/if}
		{/if}
	</div>
</div>
