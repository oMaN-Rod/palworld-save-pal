<script lang="ts">
	import { Button } from '$components/ui';
	import { FolderOpen } from 'lucide-svelte';
	import Logo from '$components/layout/Logo.svelte';
	import { SaveDropzone } from '$components/upload';

	interface Props {
		onLoad: (
			zip: Uint8Array,
			name: string,
			source?: { handle?: FileSystemDirectoryHandle; writable?: boolean }
		) => void;
		onResume?: () => void;
		resumeName?: string | null;
	}
	let { onLoad, onResume, resumeName = null }: Props = $props();
</script>

<section class="flex w-full flex-col items-center px-4 py-16 text-center sm:py-20">
	<Logo class="mb-6 w-full max-w-md" />
	<p class="text-surface-300 max-w-xl text-lg">
		The free, open-source Palworld save editor — right in your browser.
	</p>
	<div class="mt-8 w-full max-w-xl">
		<SaveDropzone {onLoad} />
	</div>
	<p class="text-surface-400 mt-4 text-sm">
		Your files never leave your device — everything runs locally in your browser.
	</p>
	{#if resumeName && onResume}
		<Button variant="secondary" class="mt-4" onclick={onResume}>
			<FolderOpen size={16} />
			Resume {resumeName}
		</Button>
	{/if}
</section>
