<script lang="ts">
	import { Loader2 } from '@lucide/svelte';
	import { Stopwatch } from '$lib/components/ui';
	import { onMount } from 'svelte';

	let {
        label = '',
		loadingComplete = $bindable(false),
        progressMessage = $bindable(''),
		icon: Icon,
		iconSize
	}: {
        label: string;
		loadingComplete?: boolean;
        progressMessage?: string;
		icon?: typeof Loader2;
		iconSize?: number;
	} = $props();
    
    let elapsed = $state(0);
    let intervalId: ReturnType<typeof setInterval>;

    onMount(() => {
		intervalId = setInterval(() => {
			elapsed += 1;
		}, 1000);

		return () => {
			if (intervalId) {
				clearInterval(intervalId);
			}
		};
	});
</script>

<div class="loading-overlay" class:loading-dismiss={loadingComplete}>
	<div class="loading-content">
		<div class="relative">
			<Loader2
				size={64}
				class="text-secondary-400 animate-spin"
				style="filter: drop-shadow(0 0 20px color-mix(in srgb, var(--color-secondary-400) 50%, transparent));"
			/>
			<Icon size={iconSize} class="text-secondary-300 absolute inset-0 m-auto" />
		</div>
		<p class="loading-text">{label}</p>
		<div class="loading-bar-track">
			<div class="loading-bar-fill" class:loading-bar-done={loadingComplete}></div>
		</div>
        {#if progressMessage}
            <span class="my-2">{progressMessage}</span>
        {/if}
        <Stopwatch class="text-secondary-400" bind:seconds={elapsed} />
	</div>
</div>

<style>
	.loading-overlay {
		position: absolute;
		inset: 0;
		z-index: 100;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--color-surface-950) 95%, transparent);
		backdrop-filter: blur(4px);
		transition:
			opacity 0.5s ease-out,
			transform 0.5s ease-out,
			filter 0.3s ease-out;
	}

	.loading-overlay.loading-dismiss {
		opacity: 0;
		transform: scale(1.05);
		filter: blur(2px);
		pointer-events: none;
	}

	.loading-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1.5rem;
		animation: loading-float 2s ease-in-out infinite;
	}

	@keyframes loading-float {
		0%,
		100% {
			transform: translateY(0);
		}
		50% {
			transform: translateY(-8px);
		}
	}

	.loading-text {
		color: color-mix(in srgb, var(--color-secondary-400) 70%, transparent);
		font-size: 0.75rem;
		letter-spacing: 0.2em;
		text-transform: uppercase;
		animation: loading-pulse 1.5s ease-in-out infinite;
		position: relative;
	}

	.loading-text::after {
		content: '';
		position: absolute;
		bottom: -4px;
		left: 0;
		width: 100%;
		height: 1px;
		background: linear-gradient(90deg, transparent, var(--color-secondary-400), transparent);
		animation: loading-scan 2s ease-in-out infinite;
	}

	@keyframes loading-pulse {
		0%,
		100% {
			opacity: 0.6;
		}
		50% {
			opacity: 1;
		}
	}

	@keyframes loading-scan {
		0% {
			transform: scaleX(0.3);
			opacity: 0;
		}
		50% {
			transform: scaleX(1);
			opacity: 1;
		}
		100% {
			transform: scaleX(0.3);
			opacity: 0;
		}
	}

	.loading-bar-track {
		width: 200px;
		height: 2px;
		background: color-mix(in srgb, var(--color-secondary-400) 15%, transparent);
		border-radius: 1px;
		overflow: hidden;
	}

	.loading-bar-fill {
		height: 100%;
		width: 0%;
		background: var(--color-secondary-400);
		border-radius: 1px;
		transition: width 0.3s ease-out;
	}

	.loading-bar-fill.loading-bar-done {
		width: 100%;
	}
</style>
