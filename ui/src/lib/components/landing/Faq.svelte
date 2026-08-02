<script lang="ts">
	import { ChevronDown } from 'lucide-svelte';

	const faqs = [
		{
			q: 'Is it safe to use on my save?',
			a: 'Your save is parsed locally in your browser. When you save back to a folder, the originals are backed up first, and writing in place is always opt-in.'
		},
		{
			q: 'Do my files get uploaded anywhere?',
			a: 'No. Everything runs in your browser. Your save never leaves your device.'
		},
		{
			q: 'Is it really free?',
			a: 'Yes. Every feature is free, with no paywall, and the whole project is open source.'
		},
		{
			q: 'Which browsers work?',
			a: 'Modern Chromium browsers and Firefox. Saving straight back to a folder needs a Chromium browser with the File System Access API.'
		},
		{
			q: 'Web version or desktop: which should I use?',
			a: 'Use the web version for zero-install editing anywhere. Use the desktop app for maximum performance and dedicated server features.'
		},
		{
			q: 'Can I trust an open-source tool with my save?',
			a: 'The code is public and auditable on GitHub, and nothing leaves your device.'
		}
	];
</script>

<section class="relative w-full overflow-hidden px-4 py-16">
	<h2 class="h2 mb-8 text-center font-bold">Questions</h2>
	<div class="mx-auto flex max-w-3xl flex-col gap-3">
		{#each faqs as item (item.q)}
			<details class="card group p-0">
				<summary
					class="flex cursor-pointer list-none items-center justify-between p-4 font-medium"
				>
					<span>{item.q}</span>
					<ChevronDown class="h-4 w-4 transition-transform group-open:rotate-180" />
				</summary>
				<p class="text-surface-300 px-4 pb-4 text-sm">{item.a}</p>
			</details>
		{/each}
	</div>
</section>
