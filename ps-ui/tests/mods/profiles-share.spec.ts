import { expect, test, type Page } from '@playwright/test';
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

// Every profile action except Activate lives behind the profile dropdown, as a
// menuitem rather than a top-level button.
async function profileAction(page: Page, name: string) {
	await page.getByRole('button', { name: en.mods_profile_actions, exact: true }).click();
	await page.getByRole('menuitem', { name, exact: true }).click();
}

let workDir: string;
let installRoot: string;
let archive: string;
let deployed: string;
let exported: string;

test.beforeAll(() => {
	workDir = realpathSync.native(
		mkdtempSync(join(process.env.PS_MODS_E2E_ROOT ?? tmpdir(), 'profiles-'))
	);
	installRoot = join(workDir, 'PalworldProfiles');
	const binaries = join(installRoot, 'Pal', 'Binaries', 'Win64');
	mkdirSync(join(installRoot, 'Pal', 'Content', 'Paks'), { recursive: true });
	mkdirSync(binaries, { recursive: true });
	writeFileSync(join(binaries, 'Palworld-Win64-Shipping.exe'), 'fake');
	writeFileSync(join(binaries, 'dwmapi.dll'), 'fake');

	archive = join(workDir, 'ProfileMod.zip');
	writeFileSync(archive, zipSync({ 'ProfileMod/Scripts/main.lua': strToU8('print("profiles")') }));

	deployed = join(binaries, 'ue4ss', 'Mods', 'ProfileMod', 'Scripts', 'main.lua');
	exported = join(workDir, 'Default.psmods');
});

test.afterAll(() => {
	rmSync(workDir, { recursive: true, force: true, maxRetries: 5 });
});

test('switches profiles, then exports one and imports it back', async ({ page }) => {
	await page.goto('/');
	await page.getByRole('link', { name: en.mods_nav, exact: true }).click();
	await expect(page).toHaveURL(/\/mods(\?|$)/);

	await page.getByRole('button', { name: en.mods_add_target, exact: true }).click();
	const addDialog = page.getByRole('dialog');
	await addDialog.getByLabel(en.mods_target_path_label).fill(installRoot);
	await addDialog.getByRole('button', { name: en.mods_target_add, exact: true }).last().click();
	await expect(addDialog).toBeHidden();
	await expect(page.getByText(installRoot).first()).toBeVisible();
	await expect(page.getByRole('button', { name: en.mods_launch, exact: true })).toHaveCount(0);
	await expect(page.getByRole('tab', { name: en.mods_tab_worlds, exact: true })).toHaveCount(0);

	await page.getByRole('button', { name: en.mods_install_open, exact: true }).click();
	const installDialog = page.getByRole('dialog');
	await installDialog.getByLabel(en.mods_install_path_label).fill(archive);
	await installDialog.getByRole('button', { name: en.mods_install_analyze, exact: true }).click();
	await installDialog.getByRole('button', { name: en.mods_install_submit, exact: true }).click();
	await expect(installDialog.getByRole('status')).toContainText(
		en.mods_panel_installed.split('{name}')[0]
	);
	await installDialog.getByRole('button', { name: en.mods_close, exact: true }).last().click();
	await expect(installDialog).toBeHidden();

	const toggle = page.getByRole('switch', { name: /profilemod/i });
	await expect(toggle).toBeChecked();
	await page.getByRole('button', { name: en.mods_panel_apply, exact: true }).click();
	await expect.poll(() => existsSync(deployed)).toBe(true);

	const profileSelect = page.getByLabel(en.mods_profile_label, { exact: true });

	await profileAction(page, en.mods_profile_new_title);
	const nameDialog = page.getByRole('dialog');
	await nameDialog.getByLabel(en.mods_profile_name_label, { exact: true }).fill('Empty');
	await nameDialog.getByRole('button', { name: en.mods_profile_create, exact: true }).click();
	await expect(nameDialog).toBeHidden();
	await expect(profileSelect.locator('option:checked')).toHaveText('Empty');
	await expect(page.getByText(en.mods_profile_inactive.replace('{name}', 'Empty'))).toBeVisible();
	await expect(toggle).not.toBeChecked();
	expect(existsSync(deployed)).toBe(true);

	await page.getByRole('button', { name: en.mods_profile_activate_label, exact: true }).click();
	await expect.poll(() => existsSync(deployed)).toBe(false);
	await expect(profileSelect.locator('option:checked')).toHaveText(
		en.mods_profile_option_active.replace('{name}', 'Empty')
	);

	await profileSelect.selectOption({ label: 'Default' });
	await page.getByRole('button', { name: en.mods_profile_activate_label, exact: true }).click();
	await expect.poll(() => existsSync(deployed)).toBe(true);

	await profileAction(page, en.mods_profile_export_label);
	const exportDialog = page.getByRole('dialog');
	await exportDialog.getByLabel(en.mods_export_path_label, { exact: true }).fill(exported);
	await exportDialog.getByRole('button', { name: en.mods_export_submit, exact: true }).click();
	await expect(exportDialog.getByText(en.mods_export_done.split('{count}')[0])).toBeVisible();
	expect(existsSync(exported)).toBe(true);
	await exportDialog.getByRole('button', { name: en.mods_close, exact: true }).last().click();
	await expect(exportDialog).toBeHidden();

	await profileAction(page, en.mods_profile_import_label);
	const importDialog = page.getByRole('dialog');
	await importDialog.getByLabel(en.mods_import_path_label, { exact: true }).fill(exported);
	await importDialog.getByRole('button', { name: en.mods_import_submit, exact: true }).click();
	await expect(
		importDialog.getByText(en.mods_import_done.replace('{name}', 'Default (2)'))
	).toBeVisible();
	await expect(importDialog.getByText(en.mods_import_pinned.replace('{count}', '1'))).toBeVisible();
	await importDialog.getByRole('button', { name: en.mods_close, exact: true }).last().click();
	await expect(importDialog).toBeHidden();

	await expect(profileSelect.locator('option:checked')).toHaveText('Default (2)');
	expect(existsSync(deployed)).toBe(true);
});
