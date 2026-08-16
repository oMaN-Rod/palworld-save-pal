<script lang="ts">
	import { onMount } from 'svelte';
	import Star from '@lucide/svelte/icons/star';
	import githubIcon from '$lib/assets/img/app/github.svg';
	import { fetchGithubStars, formatStars } from '$lib/utils/githubStars';
	import { Link } from '.';

	const GITHUB = 'https://github.com/oMaN-Rod/palworld-save-pal';

	let stars = $state<number | null>(null);
	onMount(async () => {
		stars = await fetchGithubStars();
	});
</script>

<Link href={GITHUB} label="GitHub">
	<img src={githubIcon} alt="GitHub" class="h-8 w-8" />
	{#if stars !== null}
		<span class="text-surface-200 flex items-center gap-1 text-sm font-semibold">
			<Star class="h-3.5 w-3.5 fill-current" />
			{formatStars(stars)}
		</span>
	{/if}
</Link>
