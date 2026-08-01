<script lang="ts">
	const steps = [
		{ n: 1, title: 'Drop your save', body: 'A .zip or your world folder — right on this page.' },
		{ n: 2, title: 'Edit anything', body: 'Pals, players, bases, items, blueprints and more.' },
		{ n: 3, title: 'Save it back', body: 'Write changes back to your folder, or download a copy.' }
	];
</script>

<section class="mx-auto w-full max-w-4xl px-4 py-12">
	<h2 class="h2 mb-8 text-center font-bold">How it works</h2>
	<div class="grid grid-cols-1 gap-6 sm:grid-cols-3">
		{#each steps as s (s.n)}
			<div class="flex flex-col items-center gap-2 text-center">
				<div
					class="bg-primary-500/20 text-primary-300 flex h-12 w-12 items-center justify-center rounded-full text-xl font-bold"
				>
					{s.n}
				</div>
				<h3 class="h4 font-semibold">{s.title}</h3>
				<p class="text-surface-300 text-sm">{s.body}</p>
			</div>
		{/each}
	</div>
</section>
