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

// wasm-bindgen injects these before its CLI post-processing removes them.
const allowed = /^__wbindgen/;
const foreign = imports.filter((i) => !allowed.test(i.module));

for (const i of imports) console.log(`  ${i.module}.${i.name} (${i.kind})`);

if (foreign.length > 0) {
  console.error(`\nFAIL: ${foreign.length} foreign import(s); expected 0.`);
  console.error('Add a stub to src/wasm_stubs.rs for each.');
  process.exit(1);
}
console.log(`\nOK: ${imports.length} import(s), all wasm-bindgen placeholders.`);
