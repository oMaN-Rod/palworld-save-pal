<script lang="ts">
	import { page } from '$app/state';
	import { onDestroy } from 'svelte';

	interface TocItem {
		id: string;
		text: string;
		level: number;
	}

	let tocItems = $state<TocItem[]>([]);
	let activeId = $state<string>('');
	let observer: IntersectionObserver | null = null;

	$effect(() => {
		page.url;

		observer?.disconnect();

		setTimeout(() => {
			const main = document.querySelector('main');
			if (!main) return;

			const headings = main.querySelectorAll('h2[data-toc], h3[data-toc], h4[data-toc]');

			const items: TocItem[] = [];

			headings.forEach((heading) => {
				const el = heading as HTMLElement;
				if (!el.id) {
					el.id =
						el.textContent
							?.toLowerCase()
							.replace(/[^\w\s-]/g, '')
							.replace(/\s+/g, '-') || '';
				}

				items.push({
					id: el.id,
					text: el.textContent?.trim() || '',
					level: parseInt(el.tagName[1])
				});
			});

			tocItems = items;

			observer = new IntersectionObserver(
				(entries) => {
					entries.forEach((entry) => {
						if (entry.isIntersecting) {
							activeId = entry.target.id;
						}
					});
				},
				{ root: main, rootMargin: '0px 0px -66% 0px', threshold: 0 }
			);

			headings.forEach((heading) => observer?.observe(heading));

			if (window.location.hash) {
				activeId = window.location.hash.slice(1);
			}
		}, 100);
	});

	onDestroy(() => {
		observer?.disconnect();
	});

	function scrollToSection(id: string) {
		const el = document.getElementById(id);
		if (el) {
			el.scrollIntoView({ behavior: 'smooth', block: 'start' });
			history.pushState(null, '', `#${id}`);
		}
	}
</script>

{#if tocItems.length > 0}
	<nav class="top-20 overflow-y-auto">
		<h5 class="mb-3 text-xs font-semibold uppercase tracking-wide text-surface-400">
			On This Page
		</h5>
		<ul class="space-y-0.5 text-sm">
			{#each tocItems as item}
				<li style="padding-left: {(item.level - 2) * 0.75}rem" class="relative">
					<button
						onclick={() => scrollToSection(item.id)}
						class="flex w-full items-center rounded-md px-3 py-1.5 text-left transition-colors
							{activeId === item.id
							? 'text-surface-100 bg-surface-800 font-medium'
							: 'text-surface-400 hover:bg-surface-800/50 hover:text-surface-200'}"
					>
						{#if activeId === item.id}
							<span
								class="absolute top-1/2 left-0 h-4 w-0.5 -translate-y-1/2 rounded-full bg-secondary-500"
							></span>
						{/if}
						<span>{item.text}</span>
					</button>
				</li>
			{/each}
		</ul>
	</nav>
{/if}

<style>
	nav {
		scrollbar-width: thin;
		scrollbar-color: var(--color-surface-700) transparent;
	}

	nav::-webkit-scrollbar {
		width: 4px;
	}

	nav::-webkit-scrollbar-track {
		background: transparent;
	}

	nav::-webkit-scrollbar-thumb {
		background-color: var(--color-surface-700);
		border-radius: 2px;
	}
</style>