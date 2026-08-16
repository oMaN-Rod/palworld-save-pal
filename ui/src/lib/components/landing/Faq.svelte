<script lang="ts" module>
	import * as m from '$i18n/messages';

	/** Single source for both the rendered accordion and the FAQPage schema. */
	export function faqEntries(): { question: string; answer: string }[] {
		return [
			{ question: m.landing_faq_q_safe(), answer: m.landing_faq_a_safe() },
			{ question: m.landing_faq_q_upload(), answer: m.landing_faq_a_upload() },
			{ question: m.landing_faq_q_files(), answer: m.landing_faq_a_files() },
			{ question: m.landing_faq_q_platforms(), answer: m.landing_faq_a_platforms() },
			{ question: m.landing_faq_q_free(), answer: m.landing_faq_a_free() },
			{ question: m.landing_faq_q_browsers(), answer: m.landing_faq_a_browsers() },
			{ question: m.landing_faq_q_which(), answer: m.landing_faq_a_which() },
			{ question: m.landing_faq_q_opensource(), answer: m.landing_faq_a_opensource() }
		];
	}
</script>

<script lang="ts">
	import { ChevronDown } from 'lucide-svelte';

	const faqs = $derived(faqEntries());
</script>

<section class="relative w-full px-4 py-16">
	<h2 class="heading-gradient h2 mb-8 text-center font-bold">
		{m.landing_faq_heading()}
	</h2>
	<div class="mx-auto flex max-w-3xl flex-col gap-3">
		{#each faqs as item, i (i)}
			<details class="card group p-0">
				<summary class="flex cursor-pointer list-none items-center justify-between p-4 font-medium">
					<span>{item.question}</span>
					<ChevronDown class="h-4 w-4 transition-transform group-open:rotate-180" />
				</summary>
				<p class="text-surface-300 px-4 pb-4 text-sm">{item.answer}</p>
			</details>
		{/each}
	</div>
</section>
