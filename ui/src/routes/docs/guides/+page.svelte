<script lang="ts">
	import { goto } from '$app/navigation';
	import * as m from '$i18n/messages';
	import { BookOpen, Server, HardDrive } from 'lucide-svelte';

	const guides = [
		{
			title: 'Server Setup',
			description: 'Configure and manage dedicated Palworld servers.',
			href: '/docs/guides/server-setup',
			icon: Server
		},
		{
			title: 'Save Management',
			description: 'Understand save file formats, backups, and conversions.',
			href: '/docs/guides/save-management',
			icon: HardDrive
		}
	];
</script>

<div>
	<h1>{m.docs_guides()}</h1>
	<p class="text-surface-400">{m.docs_guides_description()}</p>

	<div class="mt-6 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
		{#each guides as guide}
			<button
				onclick={() => goto( guide.href)}
				class="flex flex-col gap-2 rounded-lg border border-surface-700  p-4 no-underline transition-colors hover:border-primary-500/50 hover:bg-surface-700"
			>
				<div class="flex items-center gap-2">
					<guide.icon class="h-5 w-5 text-primary-500" />
					<h3 class="mt-0! mb-0! text-lg font-semibold">{guide.title}</h3>
				</div>
				<span class="mb-0! text-sm text-surface-400 text-left">{guide.description}</span>
			</button>
		{/each}
	</div>
</div>
