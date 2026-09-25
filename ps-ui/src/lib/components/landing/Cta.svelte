<script lang="ts">
	import discordIcon from '$lib/assets/img/app/discord.svg';
	import { GithubLink } from '.';
	import { CentrixLogo } from '$components/partners';
	import { CENTRIX } from '$lib/partners/centrixnodes';
	import * as m from '$i18n/messages';

	const DISCORD = 'https://discord.gg/YWZFPy9G8J';
</script>

<footer class="border-surface-700/50 relative w-full border-t px-4 py-14 text-center">
	<h2 class="heading-gradient h3 font-bold">{m.landing_cta_heading()}</h2>
	<div class="mt-6 flex items-center justify-center gap-5">
		<GithubLink />
		<a href={DISCORD} target="_blank" rel="noopener noreferrer" aria-label="Discord">
			<img src={discordIcon} alt="Discord" class="h-8 w-8" />
		</a>
	</div>
	<a
		href={CENTRIX.url}
		target="_blank"
		rel="noopener noreferrer"
		class="text-surface-400 hover:text-surface-200 mt-8 inline-flex flex-wrap items-center justify-center gap-x-2 gap-y-1 text-xs transition-colors"
	>
		<span>{m.landing_partner_label()}</span>
		<CentrixLogo class="w-24 opacity-80" />
		<span>{m.landing_partner_offer({ code: CENTRIX.code, discount: CENTRIX.discount })}</span>
	</a>
	<p class="text-surface-400 mt-6 text-xs">
		{m.landing_cta_footnote()}
	</p>
	<p class="text-surface-400 mt-2 text-xs">
		<a href="/privacy" class="hover:text-surface-200 underline">Privacy</a>
	</p>
</footer>
