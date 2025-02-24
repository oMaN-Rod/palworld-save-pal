import { getAppState } from './appState.svelte';

const appState = getAppState();

export type Page = 'edit' | 'info' | 'file' | 'settings' | 'loading' | 'error' | 'browser' | 'save';
export type Tab = 'player' | 'pal';

export interface NavigationState {
	activePage: Page;
	activeTab?: Tab;
}

export function createNavigationState(
	initialPage: Page = 'file',
	initialTab: Tab = 'player'
): NavigationState {
	let activePage = $state(initialPage);
	let activeTab = $state(initialTab);
	let initialLoad = true;

	function setActivePage(page: Page) {
		if (!initialLoad && page !== 'save') {
			appState.saveState();
		}
		activePage = page;
		initialLoad = false;
	}

	function setActiveTab(tab: Tab) {
		if (!initialLoad) {
			appState.saveState();
		}
		activeTab = tab;
		initialLoad = false;
	}

	return {
		get activePage() {
			return activePage;
		},
		set activePage(page: Page) {
			setActivePage(page);
		},
		get activeTab() {
			return activeTab;
		},
		set activeTab(tab: Tab) {
			setActiveTab(tab);
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
