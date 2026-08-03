<script lang="ts">
	import { browser } from '$app/environment';
	import { persistedState } from 'svelte-persisted-state';
	import { MonitorDown, TriangleAlert, X } from 'lucide-svelte';
	import * as m from '$i18n/messages';
	import {
		detectCapabilities,
		limitations,
		type LimitationKey
	} from '$lib/utils/browserCapabilities';
	import { detectBrowser } from '$lib/utils/browserIdentity';

	const DESKTOP_URL = 'https://github.com/oMaN-Rod/palworld-save-pal/releases';
	// The suggestion is suppressed on brand name as well as engine family:
	// iOS Chrome reports family 'safari' (iOS forces WebKit) but name 'Chrome',
	// and telling a Chrome user to switch to Chrome discredits the whole banner.
	// Desktop Chromium browsers already match via `family`, so this list only
	// needs the UA-string brand names that iOS-forced-WebKit browsers report.
	const CHROMIUM_NAMES = ['Chrome', 'Microsoft Edge', 'Brave', 'Opera'];

	// adapter-static prerenders this route in Node, where `Worker` and the
	// storage APIs are absent — detecting there would bake a bogus warning into
	// the shipped HTML. Only ever detect on the client.
	const losses = browser ? limitations(detectCapabilities()) : [];
	const agent = browser ? detectBrowser() : { family: 'unknown' as const, name: 'this browser' };

	const COPY: Record<LimitationKey, () => string> = {
		fsa: m.compat_loss_fsa,
		opfs: m.compat_loss_opfs,
		opfsSyncAccess: m.compat_loss_opfs_sync,
		indexedDb: m.compat_loss_indexed_db
	};

	// Re-surface the banner when the set of limitations changes rather than
	// letting an old dismissal hide a new problem. persistedState may hold a
	// legacy/absent value, so treat anything unexpected as "not dismissed".
	const signature = losses.join(',');
	const dismissed = persistedState<string>('psp-compat-dismissed', '');
	const isDismissed = $derived(dismissed.current === signature);

	// The private-window heuristic only holds for the real Chromium engine:
	// desktop Chromium reliably supports OPFS/IndexedDB outside private mode, so
	// their absence there signals private browsing. Safari-family browsers
	// (including iOS Chrome/Firefox, which iOS forces onto WebKit) can lack
	// these in ordinary browsing depending on OS version, so this stays keyed on
	// engine family, not brand name — otherwise a normal iOS user would be
	// wrongly told they're in a private window.
	const isChromiumEngine = agent.family === 'chromium';
	const showPrivateNote =
		isChromiumEngine && (losses.includes('opfs') || losses.includes('indexedDb'));

	// The "try Chromium" suggestion suppresses on brand name too, so an iOS
	// Chrome user (family 'safari', name 'Chrome') isn't told to switch to the
	// browser they're already using.
	const isChromiumBrand = isChromiumEngine || CHROMIUM_NAMES.includes(agent.name);
</script>

{#if losses.length > 0 && !isDismissed}
	<div
		class="card border-warning-500/40 bg-surface-900/95 fixed top-4 left-1/2 z-50 w-[min(28rem,calc(100vw-2rem))] -translate-x-1/2 border p-4 shadow-xl backdrop-blur"
		role="status"
	>
		<div class="flex items-start gap-3">
			<TriangleAlert class="text-warning-400 mt-0.5 h-5 w-5 shrink-0" />
			<div class="min-w-0 flex-1">
				<h3 class="h4 font-semibold">{m.compat_limited_title({ browser: agent.name })}</h3>
				<ul class="text-surface-300 mt-2 list-disc space-y-1 pl-4 text-sm">
					{#each losses as loss (loss)}
						<li>{COPY[loss]()}</li>
					{/each}
				</ul>
				{#if showPrivateNote}
					<p class="text-surface-400 mt-2 text-xs">{m.compat_private_note()}</p>
				{/if}
				<p class="text-surface-400 mt-2 text-xs">{m.compat_desktop_note()}</p>
				<div class="mt-3 flex flex-wrap items-center gap-3">
					<a
						href={DESKTOP_URL}
						target="_blank"
						rel="noopener noreferrer"
						class="btn btn-sm preset-filled-primary-500 flex items-center gap-2"
					>
						<MonitorDown size={14} />
						{m.compat_desktop_cta()}
					</a>
					{#if !isChromiumBrand}
						<span class="text-surface-400 text-xs">{m.compat_try_chromium()}</span>
					{/if}
				</div>
			</div>
			<button
				class="btn-icon btn-icon-sm shrink-0"
				aria-label={m.compat_dismiss()}
				onclick={() => (dismissed.current = signature)}
			>
				<X size={16} />
			</button>
		</div>
	</div>
{/if}
