<script lang="ts">
	import { Button } from '$components/ui';
	import { FolderOpen } from 'lucide-svelte';

	interface Props {
		onOpen: () => void;
		onResume?: () => void;
		resumeName?: string | null;
	}
	let { onOpen, onResume, resumeName = null }: Props = $props();
</script>

<section class="flex w-full flex-col items-center px-4 py-16 text-center sm:py-24">
	<h1 class="h1 max-w-3xl font-bold">Edit your Palworld saves in the browser</h1>
	<p class="text-surface-300 mt-4 max-w-xl text-lg">
		Open a save, edit pals, bases, blueprints and presets, and save it back — no install, runs
		entirely on your machine.
	</p>
	<div class="mt-8 flex flex-col items-center gap-3 sm:flex-row">
		<Button variant="primary" size="lg" class="font-bold" onclick={onOpen}>Open your save</Button>
		{#if resumeName && onResume}
			<Button variant="secondary" onclick={onResume}>
				<FolderOpen size={16} />
				Resume {resumeName}
			</Button>
		{/if}
	</div>
</section>
