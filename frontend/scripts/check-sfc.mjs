// Fail when a .vue file does not parse as a single-file component.
// `nuxt typecheck` and eslint do not compile templates, so a missing end tag
// only shows up as a runtime 500 — this keeps that out of the gate's blind spot.
import { parse } from '@vue/compiler-sfc';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path, out);
    else if (path.endsWith('.vue')) out.push(path);
  }
  return out;
}

const broken = [];
for (const file of walk('app')) {
  const { errors } = parse(readFileSync(file, 'utf8'), { filename: file });
  for (const error of errors) broken.push(`${file}: ${error.message}`);
}

if (broken.length) {
  console.error(broken.join('\n'));
  process.exit(1);
}
console.log(`sfc: ${walk('app').length} components parse`);
