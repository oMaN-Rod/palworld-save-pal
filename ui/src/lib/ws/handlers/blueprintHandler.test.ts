import { describe, expect, it, vi } from 'vitest';
import { handleExportFrame } from './blueprintHandler';

describe('handleExportFrame', () => {
	it('downloads each file when the frame is a browser array', () => {
		const download = vi.fn();
		const toast = vi.fn();
		handleExportFrame([{ name: 'Home.psp', content: 'QUJD' }], { download, toast });
		expect(download).toHaveBeenCalledWith('Home.psp', 'QUJD');
		expect(toast).not.toHaveBeenCalled();
	});

	it('toasts success when the frame is a desktop write result', () => {
		const download = vi.fn();
		const toast = vi.fn();
		handleExportFrame(
			{ message: 'Blueprint Home exported', file_path: '/x/Home.psp' },
			{ download, toast }
		);
		expect(toast).toHaveBeenCalledWith('Blueprint Home exported');
		expect(download).not.toHaveBeenCalled();
	});
});
