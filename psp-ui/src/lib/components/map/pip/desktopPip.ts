export type DesktopInvoke = (cmd: string) => Promise<unknown>;

export async function openDesktopPip(
	invoke: DesktopInvoke | undefined,
	onError: (message: string) => void
): Promise<void> {
	if (!invoke) return;
	try {
		await invoke('open_pip');
	} catch (error) {
		onError(error instanceof Error ? error.message : String(error));
	}
}
