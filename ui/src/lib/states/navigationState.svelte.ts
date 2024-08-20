export type Page = 'Edit' | 'Info' | 'File' | 'Settings' | 'Loading' | 'Error';

export interface NavigationState {
	activePage: Page;
}

export function createNavigationState(initialPage: Page): NavigationState {
	let activePage = $state(initialPage);

	return {
		get activePage() {
			return activePage;
		},
		set activePage(page: Page) {
			activePage = page;
		}
	};
}

let navigationState: ReturnType<typeof createNavigationState>;

export function getNavigationState() {
	if (!navigationState) {
		navigationState = createNavigationState('File');
	}
	return navigationState;
}
