import { describe, expect, it } from 'vitest';
import { DESKTOP_ENV, desktopEnvNeedsWrite } from './ensure-desktop-env.mjs';

const WEB_ENV = 'PUBLIC_WS_URL=\nPUBLIC_DESKTOP_MODE=false\n';

describe('desktopEnvNeedsWrite', () => {
	it('writes when the file is absent', () => {
		expect(desktopEnvNeedsWrite(null)).toBe(true);
	});

	it('leaves an existing desktop env alone', () => {
		expect(desktopEnvNeedsWrite(DESKTOP_ENV)).toBe(false);
	});

	it('keeps a customized desktop env', () => {
		const custom = 'PUBLIC_WS_URL=127.0.0.1:9999/ws\nPUBLIC_DESKTOP_MODE=true\n';
		expect(desktopEnvNeedsWrite(custom)).toBe(false);
	});

	it('repairs a web env left behind by build:web', () => {
		expect(desktopEnvNeedsWrite(WEB_ENV)).toBe(true);
	});

	it('repairs an empty ws url even in desktop mode', () => {
		expect(desktopEnvNeedsWrite('PUBLIC_WS_URL=\nPUBLIC_DESKTOP_MODE=true\n')).toBe(true);
	});

	it('repairs a missing ws url key', () => {
		expect(desktopEnvNeedsWrite('PUBLIC_DESKTOP_MODE=true\n')).toBe(true);
	});

	it('overwrites unconditionally when forced', () => {
		expect(desktopEnvNeedsWrite(DESKTOP_ENV, { force: true })).toBe(true);
	});
});
