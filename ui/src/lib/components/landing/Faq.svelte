<script lang="ts">
	import { ChevronDown } from 'lucide-svelte';
	import * as m from '$i18n/messages';

	const faqs = [
		{
			q: m.landing_faq_q_safe,
			a: m.landing_faq_a_safe
		},
		{
			q: m.landing_faq_q_upload,
			a: m.landing_faq_a_upload
		},
		{
			q: m.landing_faq_q_free,
			a: m.landing_faq_a_free
		},
		{
			q: m.landing_faq_q_browsers,
			a: m.landing_faq_a_browsers
		},
		{
			q: m.landing_faq_q_which,
			a: m.landing_faq_a_which
		},
		{
			q: m.landing_faq_q_opensource,
			a: m.landing_faq_a_opensource
		}
	];
</script>

<section class="relative w-full px-4 py-16">
	<h2 class="heading-gradient h2 mb-8 text-center font-bold">
		{m.landing_faq_heading()}
	</h2>
	<div class="mx-auto flex max-w-3xl flex-col gap-3">
		{#each faqs as item, i (i)}
			<details class="card group p-0">
				<summary
					class="flex cursor-pointer list-none items-center justify-between p-4 font-medium"
				>
					<span>{item.q()}</span>
					<ChevronDown class="h-4 w-4 transition-transform group-open:rotate-180" />
				</summary>
				<p class="text-surface-300 px-4 pb-4 text-sm">{item.a()}</p>
			</details>
		{/each}
	</div>
</section>
