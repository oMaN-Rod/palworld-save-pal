<script lang="ts">
	let { class: klass = '' }: { class?: string } = $props();
</script>

<div
	class="section-glow pointer-events-none absolute -inset-x-1/4 -top-1/4 -bottom-1/4 -z-10 {klass}"
	aria-hidden="true"
></div>

<style>
	.section-glow {
		background:
			radial-gradient(
				600px 300px at 20% 10%,
				color-mix(in srgb, var(--color-primary-400) 8%, transparent),
				transparent 70%
			),
			radial-gradient(
				500px 260px at 85% 90%,
				color-mix(in srgb, var(--color-success-600) 10%, transparent),
				transparent 72%
			);
	}
	@media (prefers-reduced-motion: no-preference) {
		.section-glow {
			animation: glow-drift 24s ease-in-out infinite alternate;
		}
	}
	@keyframes glow-drift {
		from {
			transform: translate3d(0, 0, 0);
		}
		to {
			transform: translate3d(2%, -1.5%, 0);
		}
	}
</style>
