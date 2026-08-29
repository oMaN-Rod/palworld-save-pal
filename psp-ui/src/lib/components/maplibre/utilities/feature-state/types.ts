export interface FeatureStateProps {
	source: string;
	sourceLayer?: string;
	id: string | number | null;
	state: Record<string, unknown>;
}
