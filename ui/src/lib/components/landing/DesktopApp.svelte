<script lang="ts">
	import { Monitor, ExternalLink } from 'lucide-svelte';
	import { Button } from '$components/ui';
	import SectionGlow from './SectionGlow.svelte';

	const GITHUB = 'https://github.com/oMaN-Rod/palworld-save-pal';
	const NEXUS = 'https://www.nexusmods.com/palworld/mods/1827';
</script>

<section class="relative w-full overflow-hidden px-4 py-16">
	<SectionGlow />
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
				<Button variant="primary" href={GITHUB} target="_blank" rel="noopener noreferrer">
					GitHub <ExternalLink size={16} />
				</Button>
				<Button variant="secondary" href={NEXUS} target="_blank" rel="noopener noreferrer">
					NexusMods <ExternalLink size={16} />
				</Button>
			</div>
		</div>
	</div>
</section>
