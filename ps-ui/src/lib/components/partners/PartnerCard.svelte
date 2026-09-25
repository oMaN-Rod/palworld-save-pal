<script lang="ts">
	import { CENTRIX } from '$lib/partners/centrixnodes';
	import { openExternalLink } from '$lib/utils/externalLink';
	import * as m from '$i18n/messages';
	import CentrixLogo from './CentrixLogo.svelte';
</script>

<div class="flex flex-col gap-3 px-4">
	<h4 class="h4">{m.about_partners_heading()}</h4>
	<a
		href={CENTRIX.url}
		target="_blank"
		rel="noopener noreferrer"
		class="z-10 w-fit transition-opacity hover:opacity-80"
		onclick={(event) => openExternalLink(event, CENTRIX.url)}
	>
		<CentrixLogo class="w-44" />
	</a>
	<p class="text-sm">
		{m.about_partner_offer({ code: CENTRIX.code, discount: CENTRIX.discount })}
	</p>
	<p class="text-surface-400 text-xs">{m.about_partner_disclosure()}</p>
</div>
