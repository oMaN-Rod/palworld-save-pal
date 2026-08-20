<script lang="ts">
	import { browser } from '$app/environment';
	import { persistedState } from 'svelte-persisted-state';
	import MonitorDown from '@lucide/svelte/icons/monitor-down';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import X from '@lucide/svelte/icons/x';
	import * as m from '$i18n/messages';
	import {
		detectCapabilities,
		limitations,
		type LimitationKey
	} from '$lib/utils/browserCapabilities';
	import { detectBrowser } from '$lib/utils/browserIdentity';

	const DESKTOP_URL = 'https://github.com/oMaN-Rod/palworld-save-pal/releases';
	// iOS forces WebKit, so iOS Chrome reports engine family 'safari' but name 'Chrome' — must exclude by name too.
	const CHROMIUM_NAMES = ['Chrome', 'Microsoft Edge', 'Brave', 'Opera'];

	// adapter-static prerenders this route without Worker/storage APIs; detect only on the client.
	const losses = browser ? limitations(detectCapabilities()) : [];
	const agent = browser
		? detectBrowser()
		: { family: 'unknown' as const, name: 'this browser', mobile: false };

	const COPY: Record<LimitationKey, () => string> = {
		fsa: m.compat_loss_fsa,
		opfs: m.compat_loss_opfs,
		indexedDb: m.compat_loss_indexed_db
	};

	// iOS forces WebKit on every browser, so per-capability hints like "try Chrome" don't apply to mobile.
	const isMobile = agent.mobile;

	// Re-keyed so a new limitation clears an old dismissal; mobile gets its own signature since its loss list can be empty.
	const signature = isMobile ? 'mobile' : losses.join(',');
	const dismissed = persistedState<string>('psp-compat-dismissed', '');
	const isDismissed = $derived(dismissed.current === signature);

	// Keyed on engine family, not brand: Safari-family browsers (incl. iOS-forced WebKit) can lack OPFS/IndexedDB outside private mode too, unlike real desktop Chromium.
	const isChromiumEngine = agent.family === 'chromium';
	const showPrivateNote =
		!isMobile && isChromiumEngine && (losses.includes('opfs') || losses.includes('indexedDb'));

	const isChromiumBrand = isChromiumEngine || CHROMIUM_NAMES.includes(agent.name);
	const showChromiumHint = !isMobile && !isChromiumBrand;
</script>

{#if (isMobile || losses.length > 0) && !isDismissed}
	<div
		class="card border-warning-500/40 bg-surface-900/95 fixed top-[4.5rem] left-1/2 z-50 w-[min(28rem,calc(100vw-2rem))] -translate-x-1/2 border p-4 shadow-xl backdrop-blur"
		role="status"
	>
		<div class="flex items-start gap-3">
			<TriangleAlert class="text-warning-400 mt-0.5 h-5 w-5 shrink-0" />
			<div class="min-w-0 flex-1">
				{#if isMobile}
					<h3 class="h4 font-semibold">{m.compat_mobile_title()}</h3>
					<p class="text-surface-300 mt-2 text-sm">{m.compat_mobile_body()}</p>
				{:else}
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
				{/if}
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
					{#if showChromiumHint}
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
