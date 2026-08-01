<script lang="ts">
	const faqs = [
		{
			q: 'Where do my files go?',
			a: 'Nowhere — everything runs locally in your browser. Your save never leaves your device.'
		},
		{ q: 'Do I need an account?', a: 'No. There is no sign-up and no login.' },
		{ q: 'Is it free?', a: 'Yes, always — every feature is free, with nothing behind a paywall.' },
		{
			q: 'Is it open source?',
			a: 'Yes. The full source is on GitHub — inspect it, contribute, or self-host.'
		},
		{
			q: 'Which saves are supported?',
			a: 'Palworld Steam saves (Level.sav and the Players/ folder), as a .zip or the folder itself.'
		},
		{ q: 'Is there a desktop app?', a: 'Yes — a desktop build is available from the GitHub releases.' }
	];
</script>

<section class="mx-auto w-full max-w-3xl px-4 py-12">
	<h2 class="h2 mb-8 text-center font-bold">FAQ</h2>
	<div class="flex flex-col gap-4">
		{#each faqs as f (f.q)}
			<div>
				<h3 class="h4 font-semibold">{f.q}</h3>
				<p class="text-surface-300 text-sm">{f.a}</p>
			</div>
		{/each}
	</div>
</section>
