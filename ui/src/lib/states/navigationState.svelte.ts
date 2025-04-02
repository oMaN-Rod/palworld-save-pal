import { getAppState } from './appState.svelte';

export type Page = 'edit' | 'info' | 'file' | 'settings' | 'loading' | 'error' | 'browser' | 'save';
export type Tab = 'player' | 'pal' | 'pal-box' | 'dps' | 'guilds';

export interface NavigationState {
	activePage: Page;
	activeTab?: Tab;
}

class NavigationStateManager implements NavigationState {
	#activePage = $state<Page>('file');
	#activeTab = $state<Tab>('player');
	#initialLoad = true;
	#appState = getAppState();

	constructor(initialPage: Page = 'file', initialTab: Tab = 'player') {
		this.#activePage = initialPage;
		this.#activeTab = initialTab;
	}

	get activePage(): Page {
		return this.#activePage;
	}

	set activePage(page: Page) {
		if (!this.#initialLoad && page !== 'save') {
			this.#appState.saveState();
		}
		this.#activePage = page;
		this.#initialLoad = false;
	}

	get activeTab(): Tab {
		return this.#activeTab;
	}

	set activeTab(tab: Tab) {
		if (!this.#initialLoad) {
			this.#appState.saveState();
		}
		this.#activeTab = tab;
		this.#initialLoad = false;
	}
}

const navigationStateInstance = new NavigationStateManager('file');
export const getNavigationState = () => navigationStateInstance;
