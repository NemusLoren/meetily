import { describe, expect, test } from 'bun:test';
import { parsePo } from '../../src/i18n/po';

describe('lettore dei file .po', () => {
  test('legge voci semplici, multi-riga ed escape', () => {
    const catalog = parsePo(`
msgid ""
msgstr ""
"Language: it\\n"

#: src/x.tsx:1
msgid "Save"
msgstr "Salva"

msgid "Two "
"lines"
msgstr "Due "
"righe"

msgid "Quote \\"x\\""
msgstr "Virgolette \\"x\\""

msgid "Untranslated"
msgstr ""
`);
    expect(catalog.get('Save')).toBe('Salva');
    expect(catalog.get('Two lines')).toBe('Due righe');
    expect(catalog.get('Quote "x"')).toBe('Virgolette "x"');
    expect(catalog.has('Untranslated')).toBe(false);
  });
});
