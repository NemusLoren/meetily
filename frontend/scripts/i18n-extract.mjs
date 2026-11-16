// Estrae i testi tradotti dal codice (chiamate t("...")), aggiorna messages.pot
// e fonde i testi mancanti nei file .po esistenti, senza toccare le traduzioni già scritte.
// Uso: pnpm run i18n:extract
import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..');
const require = createRequire(path.join(root, 'package.json'));
const ts = require('typescript');
const localesDir = path.join(root, 'src/i18n/locales');

function walk(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (/\.(ts|tsx)$/.test(e.name) && !/\.d\.ts$/.test(e.name)) out.push(p);
  }
  return out;
}

const found = new Map(); // msgid -> Set(riferimenti)
for (const file of walk(path.join(root, 'src'))) {
  if (file.includes(`${path.sep}i18n${path.sep}`)) continue;
  const src = fs.readFileSync(file, 'utf8');
  const sf = ts.createSourceFile(file, src, ts.ScriptTarget.Latest, true,
    file.endsWith('x') ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const visit = (node) => {
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === 't'
        && node.arguments.length > 0
        && (ts.isStringLiteral(node.arguments[0]) || ts.isNoSubstitutionTemplateLiteral(node.arguments[0]))) {
      const id = node.arguments[0].text;
      const line = sf.getLineAndCharacterOfPosition(node.getStart(sf)).line + 1;
      if (!found.has(id)) found.set(id, new Set());
      found.get(id).add(`${path.relative(root, file)}:${line}`);
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
}

const esc = (s) => s.replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\n/g, '\\n').replace(/\t/g, '\\t');
const quoted = (s) => `"${esc(s)}"`;
const ids = [...found.keys()].sort((a, b) => a.localeCompare(b));

const header = (lang) => `msgid ""\nmsgstr ""\n"Content-Type: text/plain; charset=UTF-8\\n"\n"Language: ${lang}\\n"\n`;

// .pot
let pot = header('') + '\n';
for (const id of ids) {
  pot += [...found.get(id)].slice(0, 3).map((r) => `#: ${r}`).join('\n') + `\nmsgid ${quoted(id)}\nmsgstr ""\n\n`;
}
fs.writeFileSync(path.join(localesDir, 'messages.pot'), pot);

// legge un .po esistente in mappa
function readPo(file) {
  const map = new Map();
  if (!fs.existsSync(file)) return map;
  let id = null, str = null, cur = null;
  const flush = () => { if (id !== null && str !== null && id !== '') map.set(id, str); id = null; str = null; cur = null; };
  const un = (l) => { const m = l.match(/^"(.*)"$/); return m ? m[1].replace(/\\n/g, '\n').replace(/\\t/g, '\t').replace(/\\"/g, '"').replace(/\\\\/g, '\\') : ''; };
  for (const raw of fs.readFileSync(file, 'utf8').split(/\r?\n/)) {
    const l = raw.trim();
    if (l === '') { flush(); continue; }
    if (l.startsWith('#')) continue;
    if (l.startsWith('msgid ')) { if (id !== null) flush(); id = un(l.slice(6)); cur = 'id'; }
    else if (l.startsWith('msgstr ')) { str = un(l.slice(7)); cur = 'str'; }
    else if (l.startsWith('"')) { if (cur === 'id') id += un(l); else if (cur === 'str') str += un(l); }
  }
  flush();
  return map;
}

let report = [];
for (const file of fs.readdirSync(localesDir).filter((f) => f.endsWith('.po'))) {
  const lang = file.replace('.po', '');
  if (lang === 'en') continue; // l'inglese è il testo originale
  const existing = readPo(path.join(localesDir, file));
  let out = header(lang) + '\n';
  let missing = 0;
  for (const id of ids) {
    const tr = existing.get(id) ?? '';
    if (!tr) missing++;
    out += [...found.get(id)].slice(0, 2).map((r) => `#: ${r}`).join('\n') + `\nmsgid ${quoted(id)}\nmsgstr ${quoted(tr)}\n\n`;
  }
  fs.writeFileSync(path.join(localesDir, file), out);
  report.push(`${file}: ${ids.length - missing}/${ids.length} tradotti (${missing} mancanti)`);
}
console.log(`Testi trovati: ${ids.length}\n${report.join('\n')}`);

// Controllo (pnpm run i18n:check): nessun testo mancante e segnaposto {nome} coerenti
if (process.argv.includes('--check')) {
  let problems = 0;
  for (const file of fs.readdirSync(localesDir).filter((f) => f.endsWith('.po') && f !== 'en.po')) {
    const po = readPo(path.join(localesDir, file));
    for (const id of ids) {
      const tr = po.get(id) ?? '';
      if (!tr) { console.error(`${file}: manca la traduzione di "${id}"`); problems++; continue; }
      const a = [...id.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',');
      const b = [...tr.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',');
      if (a !== b) { console.error(`${file}: segnaposto diversi in "${id}"`); problems++; }
    }
  }
  if (problems) { console.error(`${problems} problemi`); process.exit(1); }
  console.log('Controllo traduzioni: OK');
}
