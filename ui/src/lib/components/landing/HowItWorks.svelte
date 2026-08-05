<script lang="ts">
	import * as m from '$i18n/messages';

	const steps = [
		{ n: 1, title: m.landing_step_drop_title, body: m.landing_step_drop_body },
		{ n: 2, title: m.landing_step_edit_title, body: m.landing_step_edit_body },
		{ n: 3, title: m.landing_step_save_title, body: m.landing_step_save_body }
	];
</script>

<section class="relative w-full overflow-hidden px-4 py-16">
	<h2 class="h2 mb-8 text-center font-bold">{m.landing_steps_heading()}</h2>
	<div class="mx-auto grid max-w-4xl grid-cols-1 gap-6 sm:grid-cols-3">
		{#each steps as s (s.n)}
			<div class="card flex flex-col items-center gap-3 p-6 text-center">
				<div
					class="bg-primary-500 text-on-primary-token flex h-10 w-10 items-center justify-center rounded-full text-lg font-bold"
				>
					{s.n}
				</div>
				<h3 class="h4 font-semibold">{s.title()}</h3>
				<p class="text-surface-300 text-sm">{s.body()}</p>
			</div>
		{/each}
	</div>
</section>
