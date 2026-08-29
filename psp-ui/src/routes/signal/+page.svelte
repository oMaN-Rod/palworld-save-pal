<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { fade, scale } from 'svelte/transition';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { getSignalState, getToastState, rwbySkin, rwbyUnlocked } from '$states';
	import { Button, Card, Input } from '$components/ui';
	import type { SignalFeedState } from '$types';

	const signal = getSignalState();
	const toast = getToastState();

	const status = $derived(signal.status);
	const feed = $derived(status?.feed);
	const actors = $derived(status?.frame.actors ?? []);

	// -- source form ---------------------------------------------------------
	type SourceType = 'rest' | 'gamedata' | 'fake';
	let sourceType = $state<SourceType>('rest');
	let restUrl = $state('');
	let restPassword = $state('');
	let gamedataPath = $state('');

	const sourceTypes: { id: SourceType; label: string; icon: string }[] = [
		{ id: 'rest', label: 'Dedicated server', icon: 'tabler:server' },
		{ id: 'gamedata', label: 'Local game', icon: 'tabler:file-analytics' },
		{ id: 'fake', label: 'Fake', icon: 'tabler:flask' }
	];

	// -- server config form --------------------------------------------------
	let bindAddr = $state('');
	let port = $state('');
	let intervalMs = $state('');

	// Pull form defaults from the first status snapshot of this mount.
	let formInitialized = false;
	$effect(() => {
		if (!status || formInitialized) return;
		formInitialized = true;
		bindAddr = status.config.bind;
		port = String(status.config.port);
		intervalMs = String(status.config.intervalMs);
		if (status.source.kind) {
			sourceType =
				status.source.kind === 'rest' || status.source.kind === 'restgamedata'
					? 'rest'
					: (status.source.kind as 'gamedata' | 'fake');
			restUrl = status.source.url ?? restUrl;
			if (status.source.kind === 'gamedata') gamedataPath = '';
		}
	});

	onMount(() => {
		signal.startPolling(2000);
		signal.discoverGameData();
	});
	onDestroy(() => signal.stopPolling());

	const feedStateText: Record<SignalFeedState, string> = {
		idle: 'Idle',
		waiting: 'Waiting',
		auth: 'Password refused',
		down: 'Unreachable',
		stale: 'Stale',
		players: 'Players only',
		world: 'Full world',
		feeding: 'Feeding'
	};

	// The chip recipe used across the app: rounded-sm + tinted border + 500/10
	// fill + 300 text (see the breeding tab's status chips).
	const feedStateClass: Record<SignalFeedState, string> = {
		idle: 'border-surface-500/30 bg-surface-500/10 text-surface-300',
		waiting: 'border-warning-500/30 bg-warning-500/10 text-warning-300',
		auth: 'border-error-500/30 bg-error-500/10 text-error-300',
		down: 'border-error-500/30 bg-error-500/10 text-error-300',
		stale: 'border-warning-500/30 bg-warning-500/10 text-warning-300',
		players: 'border-success-500/30 bg-success-500/10 text-success-300',
		world: 'border-success-500/30 bg-success-500/10 text-success-300',
		feeding: 'border-success-500/30 bg-success-500/10 text-success-300'
	};

	const feedStateIcon: Record<SignalFeedState, string> = {
		idle: 'tabler:circle-dashed',
		waiting: 'tabler:loader-2',
		auth: 'tabler:key-off',
		down: 'tabler:plug-off',
		stale: 'tabler:clock-exclamation',
		players: 'tabler:users',
		world: 'tabler:world',
		feeding: 'tabler:broadcast'
	};

	// Segmented pill styling shared with the breeding and UPS tabs.
	function tabPill(active: boolean): string {
		return active
			? 'bg-primary-500/15 text-primary-300 border-primary-500/40 border'
			: 'text-surface-300 hover:bg-surface-800 border border-transparent';
	}

	async function handleStartStop(): Promise<void> {
		if (status?.running) {
			await signal.stop();
		} else {
			await signal.start();
		}
	}

	async function handleApplySource(): Promise<void> {
		if (sourceType === 'rest' && !restUrl.trim()) {
			toast.add('Enter the server REST address first', 'Signal', 'error');
			return;
		}
		await signal.setSource(
			sourceType === 'rest'
				? { type: 'rest', url: restUrl.trim(), password: restPassword }
				: sourceType === 'gamedata'
					? { type: 'gamedata', path: gamedataPath.trim() || undefined }
					: { type: 'fake' }
		);
		if (status && !status.running) {
			// A source with no listener serves nothing; start the API so the
			// feed is reachable the moment a source exists.
			await signal.start();
		}
	}

	async function handleSaveConfig(): Promise<void> {
		await signal.updateConfig({
			bind: bindAddr.trim() || '127.0.0.1',
			port: port ? Number(port) : undefined,
			intervalMs: intervalMs ? Number(intervalMs) : undefined,
			applyNow: true
		});
		toast.add('Signal settings saved', 'Signal', 'success');
	}

	// -- RWBY easter egg -------------------------------------------------------
	// Click the rose by the title, keep the cursor over the lore card and type
	// the name of the team — r, w, b, y in order — to unlock the reskin.
	// Unlock and skin state live in rwbyState so the palette the root layout
	// puts on <body> covers the whole WebUI, not just this tab.
	const RWBY_CODE = 'rwby';

	let loreOpen = $state(false);
	let hoveringLore = $state(false);
	let codeProgress = 0;

	function handleSecretKeydown(event: KeyboardEvent): void {
		if (!loreOpen) return;
		if (event.key === 'Escape') {
			loreOpen = false;
			return;
		}
		if (!hoveringLore || rwbyUnlocked.current) return;
		const target = event.target as HTMLElement | null;
		if (target && /^(input|textarea|select)$/i.test(target.tagName)) return;
		if (event.key.length !== 1) return;
		const key = event.key.toLowerCase();
		if (key === RWBY_CODE[codeProgress]) {
			codeProgress += 1;
			if (codeProgress === RWBY_CODE.length) {
				rwbyUnlocked.current = true;
			}
		} else {
			codeProgress = key === RWBY_CODE[0] ? 1 : 0;
		}
	}

	function toggleRwbySkin(): void {
		rwbySkin.current = !rwbySkin.current;
	}

	// Re-locks the easter egg: hides the toggle pill and reskin button and
	// drops the skin, so the secret has to be typed out all over again.
	function resetRwbySecret(): void {
		rwbyUnlocked.current = false;
		rwbySkin.current = false;
		codeProgress = 0;
		toast.add('The rose has forgotten you - type the name again to return', 'Signal', 'info');
	}
</script>

<svelte:window onkeydown={handleSecretKeydown} />

<div class="animate-fade-in flex h-full min-h-0 w-full flex-col">
	<div class="relative z-10 flex flex-1 flex-col gap-4 p-4">
		<div class="flex flex-wrap items-center justify-between gap-3">
			<div class="flex items-center gap-2">
				<div>
					<div class="flex items-center gap-1.5">
						<Icon icon="tabler:antenna" size={20} class="text-primary-400" />
						<h1 class="heading-gradient text-xl font-bold">Signal</h1>
						<button
							type="button"
							class="group shrink-0 cursor-help rounded-full transition-transform duration-200 hover:scale-125 hover:rotate-12"
							title="Why is this tab called Signal?"
							aria-label="About the Signal tab name"
							onclick={() => (loreOpen = true)}
						>
							<img
								src="/rwby-rose.webp"
								alt=""
								width="20"
								height="20"
								class="h-5 w-5 {rwbySkin.current
									? 'drop-shadow-[0_0_6px_rgb(238_52_80_/_0.8)]'
									: 'opacity-80 group-hover:opacity-100'}"
							/>
						</button>
					</div>
					<p class="text-surface-400 text-sm">
						{#if status?.source.kind}
							Broadcasting from {status.source.kind}
							{status.source.url ? `· ${status.source.url}` : ''}
						{:else}
							Broadcast the live world feed to overlay and map tools
						{/if}
					</p>
				</div>
			</div>
			<div class="flex items-center gap-2">
				{#if rwbyUnlocked.current}
					<button
						type="button"
						class="flex items-center gap-1.5 rounded-sm px-2.5 py-2 text-xs font-medium transition-all {tabPill(
							rwbySkin.current
						)}"
						title={rwbySkin.current
							? 'Return to the normal theme'
							: 'Reskin the whole app like RWBY'}
						onclick={toggleRwbySkin}
					>
						<img src="/rwby-rose.webp" alt="" width="14" height="14" class="h-3.5 w-3.5" />
						{rwbySkin.current ? 'RWBY' : 'Red like roses'}
					</button>
				{/if}
				{#if feed}
					<span
						class="flex items-center gap-1.5 rounded-sm border px-3 py-2 text-xs font-medium {feedStateClass[
							feed.state
						]}"
						title={feed.error ?? ''}
					>
						<Icon icon={feedStateIcon[feed.state]} size={13} />
						{feedStateText[feed.state]}
						{#if feed.actors > 0}
							· {feed.actors}
							{feed.actors === 1 ? 'actor' : 'actors'}
						{/if}
					</span>
				{/if}
				<Button
					variant={status?.running ? 'secondary' : 'primary'}
					size="sm"
					loading={signal.saving}
					onclick={handleStartStop}
				>
					<Icon icon={status?.running ? 'tabler:player-stop' : 'tabler:player-play'} size={14} />
					{status?.running ? 'Stop' : 'Start'}
				</Button>
			</div>
		</div>

		{#if status?.running === false && status?.config.enabled}
			<div
				class="border-warning-500/30 bg-warning-500/10 text-warning-300 flex items-center gap-1.5 rounded-sm border px-3 py-2 text-xs"
			>
				<Icon icon="tabler:alert-triangle" size={13} class="shrink-0" />
				<span>
					Signal was configured to auto-start but is not running - the port may be taken. Check the
					bind address, then press Start.
				</span>
			</div>
		{/if}

		<div class="grid min-h-0 flex-1 grid-cols-1 gap-4 xl:grid-cols-3">
			<!-- Source + feed -->
			<div class="flex min-h-0 flex-col gap-4 xl:col-span-2">
				<Card>
					<h3
						class="text-surface-400 mb-3 flex items-center gap-1.5 text-xs font-semibold tracking-wider uppercase"
					>
						<Icon icon="tabler:database-export" size={14} />
						Source
						{#if status?.source.locked}
							<span
								class="border-surface-500/30 bg-surface-500/10 text-surface-300 rounded-sm border px-2 py-0.5 normal-case"
							>
								set by the host app
							</span>
						{/if}
					</h3>

					{#if status?.source.kind}
						<div
							class="border-surface-500/30 bg-surface-500/10 mb-3 flex items-center justify-between rounded-sm border px-3 py-2 text-sm"
						>
							<span class="text-surface-300">
								Current: <span class="text-surface-50 font-medium">{status.source.kind}</span>
								{#if status.source.url}
									<span class="text-surface-400 font-mono text-xs">{status.source.url}</span>
								{/if}
							</span>
							<Button variant="secondary" size="sm" onclick={() => signal.clearSource()}>
								Forget
							</Button>
						</div>
					{/if}

					<div
						class="bg-surface-950/50 border-surface-700/40 flex gap-1 rounded-sm border p-0.5"
						role="group"
						aria-label="Source type"
					>
						{#each sourceTypes as st (st.id)}
							<button
								class="flex flex-1 items-center justify-center gap-1.5 rounded-sm px-3 py-1.5 text-xs font-medium transition-all {tabPill(
									sourceType === st.id
								)}"
								onclick={() => (sourceType = st.id)}
							>
								<Icon icon={st.icon} size={14} />
								{st.label}
							</button>
						{/each}
					</div>

					{#if sourceType === 'rest'}
						<div class="mt-3 grid gap-3 sm:grid-cols-2">
							<Input
								type="text"
								label="REST base"
								placeholder="http://127.0.0.1:8212"
								bind:value={restUrl}
							/>
							<Input
								type="password"
								label="AdminPassword"
								placeholder={status?.source.passwordSet
									? '(kept from before)'
									: 'server AdminPassword'}
								bind:value={restPassword}
							/>
						</div>
						<p class="text-surface-500 mt-2 text-xs">
							Needs RESTAPIEnabled=True on the server. The password stays in memory only - it is
							never written to disk.
						</p>
					{:else if sourceType === 'gamedata'}
						<div class="mt-3 flex flex-col gap-2">
							<Input
								type="text"
								label="GameData.json path"
								placeholder="(auto-detect from your Steam libraries)"
								bind:value={gamedataPath}
							/>
							<div class="flex flex-col gap-1">
								{#each signal.candidates as candidate (candidate.path)}
									<button
										class="hover:bg-surface-500/10 flex items-center justify-between rounded-sm p-2 text-left text-xs"
										onclick={() => (gamedataPath = candidate.path)}
									>
										<span class="text-surface-300 font-mono break-all">{candidate.path}</span>
										<span
											class="ml-2 shrink-0 rounded-sm px-1.5 py-0.5 {candidate.exists
												? 'border-success-500/30 bg-success-500/10 text-success-300 border'
												: 'border-surface-500/30 bg-surface-500/10 text-surface-400 border'}"
										>
											{candidate.origin}{candidate.exists ? ' - found' : ''}
										</span>
									</button>
								{/each}
								{#if signal.candidates.length === 0}
									<p class="text-surface-500 text-xs">
										No Steam libraries found - launch the game with -output-gamedata, or set the
										path by hand.
									</p>
								{/if}
							</div>
						</div>
					{:else}
						<p class="text-surface-500 mt-3 text-xs">
							A synthetic tamer, otomo, wild pal and base - perfect for testing a feed client
							without the game.
						</p>
					{/if}

					<div class="mt-3">
						<Button variant="primary" size="sm" loading={signal.saving} onclick={handleApplySource}>
							<Icon icon="tabler:plug-connected" size={14} /> Use this source
						</Button>
					</div>
				</Card>

				<Card class="min-h-0 flex-1">
					<div class="flex items-center justify-between">
						<h3
							class="text-surface-400 flex items-center gap-1.5 text-xs font-semibold tracking-wider uppercase"
						>
							<Icon icon="tabler:radar-2" size={14} />
							Live actors
						</h3>
						{#if status?.frame.stale}
							<span
								class="border-warning-500/30 bg-warning-500/10 text-warning-300 rounded-sm border px-3 py-2 text-xs"
							>
								stale - {Math.round(status?.frame.age ?? 0)}s old
							</span>
						{/if}
					</div>
					{#if actors.length === 0}
						<div class="text-surface-400 flex flex-col items-center justify-center gap-2 py-8">
							{#if feed?.state === 'idle'}
								<Icon icon="tabler:antenna-off" size={24} />
								<span class="text-xs">No source selected</span>
							{:else if feed?.error}
								<Icon icon="tabler:plug-off" size={24} />
								<span class="text-xs">{feed.error}</span>
							{:else}
								<Icon icon="tabler:broadcast" size={24} />
								<span class="text-xs">
									Waiting for the first frame{#if status?.running}
										- {feedStateText[feed?.state ?? 'waiting'].toLowerCase()}{/if}...
								</span>
							{/if}
						</div>
					{:else}
						<div class="max-h-72 overflow-y-auto">
							<table class="w-full text-left text-xs">
								<thead class="text-surface-500">
									<tr>
										<th class="p-1">Name</th>
										<th class="p-1">Kind</th>
										<th class="p-1">Position</th>
										<th class="p-1">Alt</th>
										<th class="p-1">Yaw</th>
										<th class="p-1">HP</th>
										<th class="p-1">Level</th>
									</tr>
								</thead>
								<tbody class="font-mono">
									{#each actors.slice(0, 50) as actor (actor.id)}
										<tr class="hover:bg-surface-500/10 border-surface-700/30 border-t">
											<td class="p-1">{actor.name ?? actor.id}</td>
											<td class="text-surface-400 p-1">{actor.kind}</td>
											<td class="p-1">{actor.x.toFixed(1)}, {actor.y.toFixed(1)}</td>
											<td class="p-1">{actor.alt.toFixed(1)}</td>
											<td class="p-1">{actor.yaw?.toFixed(0) ?? '-'}</td>
											<td class="p-1">
												{#if actor.hp !== undefined}{actor.hp}/{actor.maxHp ?? '?'}{:else}-{/if}
											</td>
											<td class="p-1">{actor.level ?? '-'}</td>
										</tr>
									{/each}
								</tbody>
							</table>
						</div>
					{/if}
				</Card>
			</div>

			<!-- Server settings -->
			<div class="flex flex-col gap-4">
				<Card>
					<h3
						class="text-surface-400 mb-3 flex items-center gap-1.5 text-xs font-semibold tracking-wider uppercase"
					>
						<Icon icon="tabler:settings-2" size={14} />
						Server
					</h3>
					{#if status}
						<p class="text-surface-500 mb-3 font-mono text-xs break-all">{status.api.url}</p>
					{/if}
					<div class="flex flex-col gap-3">
						<div class="grid grid-cols-2 gap-2">
							<Input type="text" label="Bind" placeholder="127.0.0.1" bind:value={bindAddr} />
							<Input type="number" label="Port" placeholder="8788" bind:value={port} />
						</div>
						<Input
							type="number"
							label="Poll interval (ms)"
							placeholder="1000"
							bind:value={intervalMs}
						/>
						<p class="text-surface-500 text-xs">Applies on next start.</p>
						<Button
							variant="secondary"
							size="sm"
							loading={signal.saving}
							onclick={handleSaveConfig}
						>
							<Icon icon="tabler:device-floppy" size={14} /> Save
						</Button>
					</div>
				</Card>
			</div>
		</div>
	</div>
</div>

{#if loreOpen}
	<!-- Lore overlay: the rose by the title opens it; keep the cursor over the
	     card and type the team's name to reveal the reskin. -->
	<div
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm"
		transition:fade={{ duration: 150 }}
		role="presentation"
	>
		<button
			type="button"
			class="absolute inset-0 cursor-default"
			aria-label="Close"
			onclick={() => (loreOpen = false)}
		></button>
		<div
			class="card relative z-10 flex max-w-md flex-col items-center gap-3 p-6 text-center"
			transition:scale={{ duration: 150, start: 0.95 }}
			role="dialog"
			aria-modal="true"
			aria-label="About the Signal tab name"
			tabindex="-1"
			onmouseenter={() => (hoveringLore = true)}
			onmouseleave={() => {
				hoveringLore = false;
				codeProgress = 0;
			}}
		>
			<img
				src="/rwby-rose.webp"
				alt="A flaming rose emblem"
				width="72"
				height="72"
				class="h-18 w-18 drop-shadow-[0_0_18px_rgb(238_52_80_/_0.45)]"
			/>
			<h2 class="heading-gradient text-lg font-bold">Why "Signal"?</h2>
			<p class="text-surface-300 text-sm leading-relaxed">
				Named for <span class="text-surface-50 font-medium">Signal Academy</span> - the little combat
				school on the island of Patch where Ruby Rose trained and built her sniper-scythe, Crescent Rose,
				before Ozpin bumped her to Beacon two years early. The rose beside the title is her emblem.
			</p>
			{#if rwbyUnlocked.current}
				<div class="mt-1 flex w-full flex-col items-center gap-2">
					<div class="bg-primary-500/10 border-primary-500/30 w-full rounded-sm border p-3">
						<p class="text-primary-200 text-xs font-medium">
							"They call me Little Red" - the rose recognizes you.
						</p>
					</div>
					<Button
						variant={rwbySkin.current ? 'secondary' : 'primary'}
						size="sm"
						onclick={toggleRwbySkin}
					>
						<Icon icon={rwbySkin.current ? 'tabler:arrow-back-up' : 'tabler:paint'} size={14} />
						{rwbySkin.current ? 'Back to the normal theme' : 'Reskin the app like RWBY'}
					</Button>
					<button
						type="button"
						class="text-surface-500 hover:text-surface-300 cursor-pointer text-xs underline-offset-2 transition-colors hover:underline"
						onclick={resetRwbySecret}
					>
						Hide the secret again
					</button>
				</div>
			{:else}
				<p class="text-surface-500 mt-1 text-xs italic">
					Those who know the team's name may whisper it here, letter by letter...
				</p>
			{/if}
			<button
				type="button"
				class="text-surface-500 hover:text-surface-300 absolute top-2 right-2 cursor-pointer p-1 transition-colors"
				aria-label="Close"
				onclick={() => (loreOpen = false)}
			>
				<Icon icon="tabler:x" size={16} />
			</button>
		</div>
	</div>
{/if}
