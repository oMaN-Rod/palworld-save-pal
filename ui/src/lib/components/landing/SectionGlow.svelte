<script lang="ts">
	let { class: klass = '' }: { class?: string } = $props();
</script>

<div class="section-glow pointer-events-none absolute inset-0 -z-10 {klass}" aria-hidden="true"></div>

<style>
	.section-glow {
		background:
			radial-gradient(600px 300px at 20% 10%, rgba(56, 189, 248, 0.08), transparent 70%),
			radial-gradient(500px 260px at 85% 90%, rgba(59, 125, 71, 0.1), transparent 72%);
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
