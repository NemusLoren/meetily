/**
 * Lettore minimale di file gettext `.po`.
 * Supporta msgctxt, msgid, msgstr, stringhe multi-riga e commenti. I plurali non servono:
 * il testo dell'interfaccia usa segnaposto `{nome}` e frasi che evitano il plurale.
 */
export type Catalog = Map<string, string>;

function unquote(line: string): string {
  const m = line.match(/^"(.*)"\s*$/);
  if (!m) return '';
  return m[1]
    .replace(/\\n/g, '\n')
    .replace(/\\t/g, '\t')
    .replace(/\\"/g, '"')
    .replace(/\\\\/g, '\\');
}

export function parsePo(source: string): Catalog {
  const catalog: Catalog = new Map();
  let ctx: string | null = null;
  let id: string | null = null;
  let str: string | null = null;
  let target: 'ctx' | 'id' | 'str' | null = null;

  const flush = () => {
    if (id !== null && str !== null && id !== '' && str !== '') {
      catalog.set(ctx ? `${ctx}\u0004${id}` : id, str);
    }
    ctx = null;
    id = null;
    str = null;
    target = null;
  };

  for (const raw of source.split(/\r?\n/)) {
    const line = raw.trim();
    if (line === '' || line.startsWith('#')) {
      if (line === '') flush();
      continue;
    }
    if (line.startsWith('msgctxt ')) {
      flush();
      ctx = unquote(line.slice(8));
      target = 'ctx';
    } else if (line.startsWith('msgid ')) {
      if (id !== null) flush();
      id = unquote(line.slice(6));
      target = 'id';
    } else if (line.startsWith('msgstr ')) {
      str = unquote(line.slice(7));
      target = 'str';
    } else if (line.startsWith('"')) {
      const piece = unquote(line);
      if (target === 'ctx') ctx = (ctx ?? '') + piece;
      else if (target === 'id') id = (id ?? '') + piece;
      else if (target === 'str') str = (str ?? '') + piece;
    }
  }
  flush();
  return catalog;
}
