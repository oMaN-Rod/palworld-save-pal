<script lang="ts">
	import { Monitor } from 'lucide-svelte';
	import { GithubLink, NexusModsLink } from '.';
	
</script>

<section class="relative w-full overflow-hidden px-4 py-16">
	<div
		class="card mx-auto flex max-w-4xl flex-col items-center gap-6 p-8 sm:flex-row sm:text-left"
	>
		<Monitor class="text-primary-400 h-16 w-16 shrink-0" />
		<div class="text-center sm:text-left">
			<h2 class="h3 font-bold">Prefer the desktop app? It is here to stay</h2>
			<p class="text-surface-300 mt-2">
				The desktop version stays free and maintained. Choose it for maximum performance and for
				dedicated server features the browser cannot reach.
			</p>
			<div class="mt-5 flex flex-wrap justify-center gap-3 sm:justify-start">
				<GithubLink />
				<NexusModsLink />
			</div>
		</div>
	</div>
</section>
