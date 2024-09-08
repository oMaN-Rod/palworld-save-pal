export type Page = 'edit' | 'info' | 'file' | 'settings' | 'loading' | 'error' | 'browser';
export type Tab = 'player' | 'pal';

export interface NavigationState {
	activePage: Page;
	activeTab?: Tab;
}

export function createNavigationState(initialPage: Page = 'file', initialTab: Tab = 'player'): NavigationState {
	let activePage = $state(initialPage);
	let activeTab = $state(initialTab);

	return {
		get activePage() {
			return activePage;
		},
		set activePage(page: Page) {
			activePage = page;
		},
		get activeTab() {
			return activeTab;
		},
		set activeTab(tab: Tab) {
			activeTab = tab;
		}
	};
}

let navigationState: ReturnType<typeof createNavigationState>;

export function getNavigationState() {
	if (!navigationState) {
		navigationState = createNavigationState('file');
	}
	return navigationState;
}
