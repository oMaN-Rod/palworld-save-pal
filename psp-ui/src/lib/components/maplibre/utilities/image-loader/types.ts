import type { Snippet } from 'svelte';

export interface ImageLoaderProps {
	images: Record<string, string>;
	loading?: boolean;
	onerror?: (id: string, url: string, error: unknown) => void;
	children?: Snippet;
}
