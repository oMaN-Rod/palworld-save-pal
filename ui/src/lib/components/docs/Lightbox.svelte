<script lang="ts">
	let {
		src = $bindable(''),
		alt = $bindable('')
	}: { src: string; alt: string } = $props();

	let open = $derived(src !== '');

	function close() {
		src = '';
		alt = '';
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') close();
	}
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
	<div class="lightbox-overlay" onclick={close}>
		<button class="lightbox-close" onclick={close} aria-label="Close lightbox">&times;</button>
		<img {src} {alt} class="lightbox-image" onclick={(e) => e.stopPropagation()} />
		{#if alt}
			<span class="lightbox-caption">{alt}</span>
		{/if}
	</div>
{/if}

<style>
	.lightbox-overlay {
		position: fixed;
		inset: 0;
		z-index: 9999;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		background: rgba(0, 0, 0, 0.85);
		backdrop-filter: blur(4px);
		cursor: zoom-out;
	}

	.lightbox-close {
		position: absolute;
		top: 1rem;
		right: 1rem;
		background: none;
		border: none;
		color: var(--color-surface-200);
		font-size: 2rem;
		cursor: pointer;
		line-height: 1;
		padding: 0.25rem 0.5rem;
		border-radius: 0.25rem;
		transition: color 0.15s;
	}

	.lightbox-close:hover {
		color: white;
	}

	.lightbox-image {
		max-width: 90vw;
		max-height: 85vh;
		object-fit: contain;
		border-radius: 0.5rem;
		cursor: default;
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
	}

	.lightbox-caption {
		margin-top: 0.75rem;
		color: var(--color-surface-300);
		font-size: 0.875rem;
		text-align: center;
	}
</style>
