<script lang="ts">
	import githubIcon from '$lib/assets/img/app/github.svg';
	import discordIcon from '$lib/assets/img/app/discord.svg';
</script>

<footer
	class="border-surface-700/50 mt-8 flex w-full flex-col items-center gap-4 border-t px-4 py-10 text-center"
>
	<div class="flex items-center gap-6">
		<a
			href="https://github.com/oMaN-Rod/palworld-save-pal"
			target="_blank"
			rel="noopener noreferrer"
			aria-label="GitHub"
		>
			<img src={githubIcon} alt="GitHub" class="h-8 w-8" />
		</a>
		<a href="https://discord.gg/YWZFPy9G8J" target="_blank" rel="noopener noreferrer" aria-label="Discord">
			<img src={discordIcon} alt="Discord" class="h-8 w-8" />
		</a>
	</div>
	<p class="text-surface-400 text-xs">
		Free and open source. A fan-made tool, not affiliated with Pocketpair, Inc.
	</p>
</footer>
