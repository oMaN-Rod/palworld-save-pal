<script lang="ts">
	import { page } from '$app/state';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import LocaleChip from './LocaleChip.svelte';
	import ThemeChip from './ThemeChip.svelte';
	import { publicNavItems, activePublicNavId } from './publicNavItems';
	import { browser } from '$app/environment';
	import { detectBrowser } from '$lib/utils/browserIdentity';
	import { layout } from '$lib/utils/layout.svelte';

	const activeId = $derived(activePublicNavId(page.url.pathname));

	const agent = browser
		? detectBrowser()
		: { family: 'unknown' as const, name: 'this browser', mobile: false };
	const isMobile = $derived(agent.mobile || layout.phone);

	const navItems = $derived(publicNavItems.filter((item) => !(isMobile && item.hideOnMobile)));
</script>

<nav class="public-nav">
	<a href="/" class="public-nav-brand" aria-label="PalStudio home">
		<img src="/ps.png" alt="" class="h-5 w-5 rounded object-contain" />
		<span class="heading-gradient hidden text-xs font-extrabold tracking-tight md:inline">
			PALSTUDIO
		</span>
	</a>

	<div class="public-nav-links">
		{#each navItems as item (item.id)}
			<a
				href={item.href}
				class="public-nav-link"
				class:is-active={activeId === item.id}
				aria-current={activeId === item.id ? 'page' : undefined}
				aria-label={item.label()}
			>
				<Icon icon={item.icon} class="h-4 w-4 shrink-0" />
				<span class="public-nav-label hidden sm:inline">{item.label()}</span>
			</a>
		{/each}
	</div>

	<div class="public-nav-chips">
		<LocaleChip />
		<ThemeChip />
	</div>
</nav>
