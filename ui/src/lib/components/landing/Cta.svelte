<script lang="ts">
	import discordIcon from '$lib/assets/img/app/discord.svg';
	import SectionGlow from './SectionGlow.svelte';
	import { GithubLink } from '.';

	const DISCORD = 'https://discord.gg/YWZFPy9G8J';
</script>

<footer class="border-surface-700/50 relative w-full overflow-hidden border-t px-4 py-14 text-center">
	<SectionGlow />
	<h2 class="h3 font-bold">Ready to jump in?</h2>
	<div class="mt-6 flex items-center justify-center gap-5">
		<GithubLink />
		<a href={DISCORD} target="_blank" rel="noopener noreferrer" aria-label="Discord">
			<img src={discordIcon} alt="Discord" class="h-8 w-8" />
		</a>
	</div>
	<p class="text-surface-400 mt-6 text-xs">
		Free and open source. A fan-made tool, not affiliated with Pocketpair, Inc.
	</p>
</footer>
