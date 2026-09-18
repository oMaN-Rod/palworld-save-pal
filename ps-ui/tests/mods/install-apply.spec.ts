import { expect, test } from '@playwright/test';
import { strToU8, zipSync } from 'fflate';
import {
	existsSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	realpathSync,
	rmSync,
	writeFileSync
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const en: Record<string, string> = JSON.parse(
	readFileSync(
		join(dirname(fileURLToPath(import.meta.url)), '../../../data/json/ui/en.json'),
		'utf8'
	)
);

function countPattern(key: string): RegExp {
	const escaped = en[key].replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
	return new RegExp(escaped.replace('\\{count\\}', '\\d+'));
}

let workDir: string;
let installRoot: string;
let archive: string;
let deployed: string;

test.beforeAll(() => {
	workDir = realpathSync.native(
		mkdtempSync(join(process.env.PS_MODS_E2E_ROOT ?? tmpdir(), 'fixture-'))
	);
	installRoot = join(workDir, 'Palworld');
	const binaries = join(installRoot, 'Pal', 'Binaries', 'Win64');
	mkdirSync(join(installRoot, 'Pal', 'Content', 'Paks'), { recursive: true });
	mkdirSync(binaries, { recursive: true });
	writeFileSync(join(binaries, 'Palworld-Win64-Shipping.exe'), 'fake');
	writeFileSync(join(binaries, 'dwmapi.dll'), 'fake');

	archive = join(workDir, 'CoolMod.zip');
	writeFileSync(archive, zipSync({ 'CoolMod/Scripts/main.lua': strToU8('print("hi")') }));

	deployed = join(binaries, 'ue4ss', 'Mods', 'CoolMod', 'Scripts', 'main.lua');
});

test.afterAll(() => {
	rmSync(workDir, { recursive: true, force: true, maxRetries: 5 });
});

test('installs, enables and applies a UE4SS mod, then removes it when disabled', async ({
	page
}) => {
	await page.goto('/');
	await page.getByRole('link', { name: en.mods_nav, exact: true }).click();
	await expect(page).toHaveURL(/\/mods(\?|$)/);

	await page.getByRole('button', { name: en.mods_add_target, exact: true }).click();
	const addDialog = page.getByRole('dialog');
	await addDialog.getByLabel(en.mods_target_path_label).fill(installRoot);
	await addDialog.getByRole('button', { name: en.mods_target_add, exact: true }).last().click();
	await expect(addDialog).toBeHidden();
	await expect(page.getByText(installRoot).first()).toBeVisible();

	await page.getByRole('button', { name: en.mods_install_open, exact: true }).click();
	const installDialog = page.getByRole('dialog');
	await installDialog.getByLabel(en.mods_install_path_label).fill(archive);
	await installDialog.getByRole('button', { name: en.mods_install_analyze, exact: true }).click();

	const groups = installDialog.getByRole('button', { expanded: false });
	await expect(groups).toHaveCount(1);
	await expect(groups).toContainText(en.mods_review_dest_ue4ss);
	await groups.click();
	await expect(installDialog.getByText('main.lua', { exact: true })).toBeVisible();

	await installDialog.getByRole('button', { name: en.mods_install_submit, exact: true }).click();
	await expect(installDialog.getByRole('status')).toContainText(
		en.mods_panel_installed.split('{name}')[0]
	);
	await installDialog.getByRole('button', { name: en.mods_close, exact: true }).last().click();
	await expect(installDialog).toBeHidden();

	const toggle = page.getByRole('switch', { name: /coolmod/i });
	await expect(toggle).toBeChecked();

	const apply = page.getByRole('button', { name: en.mods_panel_apply, exact: true });
	const pendingChanges = page.getByText(countPattern('mods_apply_pending'));
	const upToDate = page.getByText(en.mods_apply_up_to_date, { exact: true });

	await expect(pendingChanges).toBeVisible();
	await apply.click();
	await expect(upToDate).toBeVisible();
	expect(existsSync(deployed)).toBe(true);
	expect(readFileSync(deployed, 'utf8')).toBe('print("hi")');

	await toggle.click();
	await expect(toggle).not.toBeChecked();
	await expect.poll(() => existsSync(deployed)).toBe(false);
	await expect(upToDate).toBeVisible();
	// With nothing left to apply the floating bar unmounts, so there is no
	// Apply button to press rather than a disabled one.
	await expect(apply).toBeHidden();
});
