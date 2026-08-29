<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import * as m from '$i18n/messages';

	const values = [
		{ icon: 'tabler:gift', title: m.landing_value_free_title, body: m.landing_value_free_body },
		{
			icon: 'tabler:code',
			title: m.landing_value_opensource_title,
			body: m.landing_value_opensource_body
		},
		{
			icon: 'tabler:devices',
			title: m.landing_value_device_title,
			body: m.landing_value_device_body
		},
		{
			icon: 'tabler:shield-check',
			title: m.landing_value_privacy_title,
			body: m.landing_value_privacy_body
		}
	];
</script>

<section class="relative w-full px-4 py-16">
	<h2 class="heading-gradient h2 mb-8 text-center font-bold">
		{m.landing_values_heading()}
	</h2>
	<div class="mx-auto grid max-w-5xl grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-4">
		{#each values as v (v.title)}
			<div class="card flex flex-col items-center gap-3 p-6 text-center">
				<Icon icon={v.icon} class="text-primary-400 h-8 w-8" />
				<h3 class="h4 font-semibold">{v.title()}</h3>
				<p class="text-surface-300 text-sm">{v.body()}</p>
			</div>
		{/each}
	</div>
</section>
