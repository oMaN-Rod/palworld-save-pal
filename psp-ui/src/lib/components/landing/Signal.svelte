<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import * as m from '$i18n/messages';

	const GUIDE = '/docs/guides/remote-access';

	const steps = [
		{ n: 1, title: m.landing_signal_step_arm_title, body: m.landing_signal_step_arm_body },
		{ n: 2, title: m.landing_signal_step_pair_title, body: m.landing_signal_step_pair_body },
		{ n: 3, title: m.landing_signal_step_watch_title, body: m.landing_signal_step_watch_body }
	];

	const features = [
		{
			icon: 'tabler:activity',
			title: m.landing_signal_feature_live_title,
			body: m.landing_signal_feature_live_body
		},
		{
			icon: 'tabler:edit',
			title: m.landing_signal_feature_edit_title,
			body: m.landing_signal_feature_edit_body
		},
		{
			icon: 'tabler:server',
			title: m.landing_signal_feature_servers_title,
			body: m.landing_signal_feature_servers_body
		},
		{
			icon: 'tabler:picture-in-picture-on',
			title: m.landing_signal_feature_minimap_title,
			body: m.landing_signal_feature_minimap_body
		}
	];

	const points = [
		{
			icon: 'tabler:lock',
			title: m.landing_signal_point_p2p_title,
			body: m.landing_signal_point_p2p_body
		},
		{
			icon: 'tabler:eye-off',
			title: m.landing_signal_point_broker_title,
			body: m.landing_signal_point_broker_body
		},
		{
			icon: 'tabler:shield-check',
			title: m.landing_signal_point_control_title,
			body: m.landing_signal_point_control_body
		}
	];
</script>

<section class="relative w-full px-4 py-16 text-center">
	<Icon icon="tabler:device-mobile" class="text-primary-400 mx-auto h-10 w-10" />
	<h2 class="heading-gradient h2 mx-auto mt-3 max-w-3xl font-bold">
		{m.landing_signal_heading()}
	</h2>
	<p class="text-surface-300 mx-auto mt-4 max-w-2xl">
		{m.landing_signal_body()}
	</p>

	<div class="mx-auto mt-10 grid max-w-5xl grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-4">
		{#each features as f (f.icon)}
			<div class="card flex flex-col items-center gap-3 p-6">
				<Icon icon={f.icon} class="text-primary-400 h-8 w-8" />
				<h3 class="h4 font-semibold">{f.title()}</h3>
				<p class="text-surface-300 text-sm">{f.body()}</p>
			</div>
		{/each}
	</div>

	<div class="mx-auto mt-10 grid max-w-4xl grid-cols-1 gap-6 sm:grid-cols-3">
		{#each steps as s (s.n)}
			<div class="card flex flex-col items-center gap-3 p-6">
				<div
					class="bg-primary-500 text-primary-contrast-500 flex h-10 w-10 items-center justify-center rounded-full text-lg font-bold"
				>
					{s.n}
				</div>
				<h3 class="h4 font-semibold">{s.title()}</h3>
				<p class="text-surface-300 text-sm">{s.body()}</p>
			</div>
		{/each}
	</div>

	<div class="mx-auto mt-8 grid max-w-4xl grid-cols-1 gap-6 sm:grid-cols-3">
		{#each points as p (p.icon)}
			<div class="flex flex-col items-center gap-2">
				<Icon icon={p.icon} class="text-primary-400 h-7 w-7" />
				<h3 class="text-surface-100 font-semibold">{p.title()}</h3>
				<p class="text-surface-300 text-sm">{p.body()}</p>
			</div>
		{/each}
	</div>

	<p class="text-surface-400 mt-8 text-sm">{m.landing_signal_note()}</p>
	<a
		href={GUIDE}
		class="text-primary-400 mt-2 inline-flex items-center gap-1 text-sm hover:underline"
	>
		{m.landing_signal_guide()}
		<Icon icon="tabler:arrow-right" size={14} />
	</a>
</section>
