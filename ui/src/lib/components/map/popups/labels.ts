/** Fallback title for feature types without a dedicated hover/popup: `fast_travel` -> `Fast Travel`. */
export function featureTypeLabel(type: string): string {
	return type
		.split('_')
		.filter((word) => word.length > 0)
		.map((word) => word[0].toUpperCase() + word.slice(1))
		.join(' ');
}
