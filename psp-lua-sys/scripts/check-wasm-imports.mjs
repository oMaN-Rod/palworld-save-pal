// Verifies a built .wasm imports nothing outside wasm-bindgen's own placeholders.
// Usage: node scripts/check-wasm-imports.mjs path/to/module.wasm
import fs from 'node:fs';

const path = process.argv[2];
if (!path) {
  console.error('usage: node check-wasm-imports.mjs <module.wasm>');
  process.exit(2);
}

const module = new WebAssembly.Module(fs.readFileSync(path));
const imports = WebAssembly.Module.imports(module);

// For --target web, every wasm-bindgen import is satisfied by the JS glue
// file emitted alongside the module (module name ends in "_bg.js"); anything
// imported from another module (env, wasi_snapshot_preview1, ...) is foreign.
const allowed = /_bg\.js$/;
const foreign = imports.filter((i) => !allowed.test(i.module));

for (const i of imports) console.log(`  ${i.module}.${i.name} (${i.kind})`);

if (foreign.length > 0) {
  console.error(`\nFAIL: ${foreign.length} foreign import(s); expected 0.`);
  console.error('Add a stub to src/wasm_stubs.rs for each.');
  process.exit(1);
}
console.log(`\nOK: ${imports.length} import(s), all wasm-bindgen placeholders.`);
