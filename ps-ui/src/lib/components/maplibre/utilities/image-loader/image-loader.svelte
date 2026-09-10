<script lang="ts">
	import { onDestroy } from 'svelte';
	import { getMapContext } from '../../contexts.svelte.js';
	import type { ImageLoaderProps } from './types.js';

	let { images, loading = $bindable(true), onerror, children }: ImageLoaderProps = $props();

	const ctx = getMapContext();

	const addedImages = new Set<string>();
	let generation = 0;

	$effect(() => {
		const map = ctx.map;
		const loaded = ctx.loaded;
		if (!map || !loaded) return;

		const currentImages = images;
		const gen = ++generation;

		loading = true;

		for (const id of addedImages) {
			if (!(id in currentImages)) {
				try {
					if (map.hasImage(id)) map.removeImage(id);
				} catch {
					// ignore
				}
				addedImages.delete(id);
			}
		}

		const toLoad: Array<{ id: string; url: string }> = [];
		for (const [id, url] of Object.entries(currentImages)) {
			if (!map.hasImage(id)) {
				toLoad.push({ id, url });
			}
		}

		if (toLoad.length === 0) {
			loading = false;
			return;
		}

		Promise.all(
			toLoad.map(async ({ id, url }) => {
				try {
					const response = await map.loadImage(url);
					return { id, data: response.data, error: null };
				} catch (err) {
					return { id, url, data: null, error: err };
				}
			})
		).then((results) => {
			if (gen !== generation) return;

			for (const result of results) {
				if (result.error) {
					console.warn(`svlibre: ImageLoader failed to load "${result.id}"`, result.error);
					if (onerror) onerror(result.id, (result as { url: string }).url, result.error);
					continue;
				}
				try {
					if (!map.hasImage(result.id)) {
						map.addImage(result.id, result.data!);
						addedImages.add(result.id);
					}
				} catch (err) {
					console.warn(`svlibre: ImageLoader failed to add "${result.id}"`, err);
					if (onerror) onerror(result.id, '', err);
				}
			}

			loading = false;
		});
	});

	onDestroy(() => {
		const map = ctx.map;
		if (!map) return;
		for (const id of addedImages) {
			try {
				if (map.hasImage(id)) map.removeImage(id);
			} catch {
				// Map may be destroyed
			}
		}
		addedImages.clear();
	});
</script>

{#if !loading}
	{@render children?.()}
{/if}
