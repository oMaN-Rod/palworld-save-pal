<script lang="ts">
	import { Card } from '$components/ui';
	import { Gift, GitBranch, MonitorSmartphone, ShieldCheck } from 'lucide-svelte';

	const values = [
		{ icon: Gift, title: 'Free forever', body: 'Every feature, free — nothing behind a paywall.' },
		{ icon: GitBranch, title: 'Open source', body: 'Fully open — inspect, contribute, or self-host.' },
		{
			icon: MonitorSmartphone,
			title: '100% on your device',
			body: 'No upload, no account — your save stays on your machine.'
		},
		{ icon: ShieldCheck, title: 'Privacy & security first', body: 'Nothing is collected or sent anywhere.' }
	];
</script>

<section class="mx-auto grid w-full max-w-4xl grid-cols-1 gap-6 px-4 py-12 sm:grid-cols-2">
	{#each values as v (v.title)}
		<Card class="flex gap-4">
			<v.icon class="text-primary-400 h-8 w-8 shrink-0" />
			<div>
				<h3 class="h4 font-semibold">{v.title}</h3>
				<p class="text-surface-300 text-sm">{v.body}</p>
			</div>
		</Card>
	{/each}
</section>
