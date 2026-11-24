import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import MarkdownIt from 'markdown-it';
import { INTERNAL_LINK_REGEX, internalLinkPlugin } from './internal-links';

const md = new MarkdownIt();
md.use(internalLinkPlugin);

const UUID = '01a206e9-58a4-77d0-a285-2dab6290a7d6';
const env = { resolveNode: (id: string) => (id === UUID ? 'Target note' : undefined) };

function renderInline(str: string): string {
  return md.renderInline(str, env);
}

describe('markdown internal links', () => {
  test('uuid linkifies with the resolved name', () => {
    assert.equal(
      renderInline(`see #${UUID} now`),
      `see <a href="/dashboard/docs/${UUID}" class="internal-link" data-internal-link="${UUID}">#Target note</a> now`,
    );
  });

  test('unknown uuid falls back to the raw id label', () => {
    const other = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee';
    assert.equal(
      renderInline(`#${other}`),
      `<a href="/dashboard/docs/${other}" class="internal-link" data-internal-link="${other}">#${other}</a>`,
    );
  });

  test('legacy 15-20 digit ids still linkify', () => {
    assert.match(renderInline('#726782907684193440'), /data-internal-link="726782907684193440"/);
  });

  test('uppercase hex uuid matches', () => {
    const upper = UUID.toUpperCase();
    assert.match(renderInline(`#${upper}`), new RegExp(`data-internal-link="${upper}"`));
  });

  test('short ids, uuid prefixes and word-glued hashes pass through', () => {
    assert.equal(renderInline('#123'), '#123');
    assert.equal(renderInline('#01a2'), '#01a2');
    assert.equal(renderInline(`a#${UUID}`), `a#${UUID}`);
    assert.equal(renderInline(`#${'1'.repeat(21)}`), `#${'1'.repeat(21)}`);
  });

  test('global regex finds every link in a scan', () => {
    const ids = [...`#${UUID} and #726782907684193440`.matchAll(INTERNAL_LINK_REGEX)].map(m => m[1]);
    assert.deepEqual(ids, [UUID, '726782907684193440']);
  });
});
