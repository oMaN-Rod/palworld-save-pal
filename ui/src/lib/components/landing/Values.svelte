<script lang="ts">
	import { Gift, Code, MonitorSmartphone, ShieldCheck } from 'lucide-svelte';
	import SectionGlow from './SectionGlow.svelte';

	const values = [
		{ icon: Gift, title: 'Free forever', body: 'Every feature, no paywall, ever.' },
		{ icon: Code, title: 'Open source', body: 'Community built and fully auditable on GitHub.' },
		{
			icon: MonitorSmartphone,
			title: '100% on your device',
			body: 'Your save is parsed in your browser. Nothing is uploaded.'
		},
		{
			icon: ShieldCheck,
			title: 'Privacy and security first',
			body: 'No accounts, no tracking, no servers holding your data.'
		}
	];
</script>

<section class="relative w-full overflow-hidden px-4 py-16">
	<h2 class="h2 mb-8 text-center font-bold">Built different, on purpose</h2>
	<div class="mx-auto grid max-w-5xl grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-4">
		{#each values as v (v.title)}
			<div class="card flex flex-col items-center gap-3 p-6 text-center">
				<v.icon class="text-primary-400 h-8 w-8" />
				<h3 class="h4 font-semibold">{v.title}</h3>
				<p class="text-surface-300 text-sm">{v.body}</p>
			</div>
		{/each}
	</div>
</section>
