<script lang="ts">
	import Monitor from '@lucide/svelte/icons/monitor';
	import { GithubLink, NexusModsLink } from '.';
	import * as m from '$i18n/messages';
</script>

<section class="relative w-full px-4 py-16">
	<div class="card mx-auto flex max-w-4xl flex-col items-center gap-6 p-8 sm:flex-row sm:text-left">
		<Monitor class="text-primary-400 h-16 w-16 shrink-0" />
		<div class="text-center sm:text-left">
			<h2 class="heading-gradient h3 font-bold">{m.landing_desktop_heading()}</h2>
			<p class="text-surface-300 mt-2">
				{m.landing_desktop_body()}
			</p>
			<div class="mt-5 flex flex-wrap justify-center gap-3 sm:justify-start">
				<GithubLink />
				<NexusModsLink />
			</div>
		</div>
	</div>
</section>
