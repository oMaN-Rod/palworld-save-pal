import { goto } from '$app/navigation';
import { page } from '$app/state';
import { getAppState } from './appState.svelte';

export type Tab = 'player' | 'pal' | 'pal-box' | 'dps' | 'guilds' | 'gps' | 'technologies';

export interface NavigationState {
	activePage: string;
	activeTab?: Tab;
}

class NavigationStateManager implements NavigationState {
	#activePage = $state('file');
	#activeTab = $state<Tab>('pal');
	#initialLoad = true;
	#appState = getAppState();
	constructor(initialPage: string = 'file', initialTab: Tab = 'pal') {
		this.#activePage = initialPage;
		this.#activeTab = initialTab;
	}

	isCurrentPath(path: string): boolean {
		return page.url.pathname === path;
	}

	navigateTo(page: string): void {
		this.#activePage = page;
	}

	saveAndNavigate(page: string): void {
		if (!this.#initialLoad && page !== 'save') {
			this.#appState.saveState();
		}
		this.#activePage = page;
		this.#initialLoad = false;
		goto(page);
	}

	get activePage(): string {
		return this.#activePage;
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
