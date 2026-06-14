class DocsState {
	searchQuery = $state('');
	activeCategory = $state('');
}

const docsStateInstance = new DocsState();
export const getDocsState = () => docsStateInstance;
