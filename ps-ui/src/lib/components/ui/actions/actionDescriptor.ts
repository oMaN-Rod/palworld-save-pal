/** `label` is shown as text, never as a hover-only tooltip. */
export type ActionDescriptor = {
	id: string;
	label: string;
	/** Iconify name, e.g. `tabler:sort-ascending`. */
	icon: string;
	run: () => void | Promise<void>;
	available?: () => boolean;
	/** Renders in the error palette and sorts last in the sheet. */
	danger?: boolean;
	/** Trailing value shown in the sheet row, e.g. a current quantity. */
	detail?: () => string;
};

export function availableActions(actions: ActionDescriptor[]): ActionDescriptor[] {
	return actions.filter((action) => action.available?.() ?? true);
}

/** Danger sorts last, away from the resting thumb; rails keep declared order. */
export function sheetActions(actions: ActionDescriptor[]): ActionDescriptor[] {
	return [...availableActions(actions)].sort(
		(a, b) => Number(a.danger ?? false) - Number(b.danger ?? false)
	);
}
