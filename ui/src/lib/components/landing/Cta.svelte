<script lang="ts">
	import { onMount } from 'svelte';
	import { Star } from 'lucide-svelte';
	import githubIcon from '$lib/assets/img/app/github.svg';
	import discordIcon from '$lib/assets/img/app/discord.svg';
	import { fetchGithubStars, formatStars } from '$lib/utils/githubStars';
	import SectionGlow from './SectionGlow.svelte';

	const GITHUB = 'https://github.com/oMaN-Rod/palworld-save-pal';
	const DISCORD = 'https://discord.gg/YWZFPy9G8J';

	let stars = $state<number | null>(null);
	onMount(async () => {
		stars = await fetchGithubStars();
	});
</script>

<footer class="border-surface-700/50 relative w-full overflow-hidden border-t px-4 py-14 text-center">
	<SectionGlow />
	<h2 class="h3 font-bold">Ready to jump in?</h2>
	<div class="mt-6 flex items-center justify-center gap-5">
		<a
			href={GITHUB}
			target="_blank"
			rel="noopener noreferrer"
			aria-label="GitHub"
			class="flex items-center gap-2"
		>
			<img src={githubIcon} alt="GitHub" class="h-8 w-8" />
			{#if stars !== null}
				<span class="text-surface-200 flex items-center gap-1 text-sm font-semibold">
					<Star class="h-3.5 w-3.5 fill-current" />
					{formatStars(stars)}
				</span>
			{/if}
		</a>
		<a href={DISCORD} target="_blank" rel="noopener noreferrer" aria-label="Discord">
			<img src={discordIcon} alt="Discord" class="h-8 w-8" />
		</a>
	</div>
	<p class="text-surface-400 mt-6 text-xs">
		Free and open source. A fan-made tool, not affiliated with Pocketpair, Inc.
	</p>
</footer>
