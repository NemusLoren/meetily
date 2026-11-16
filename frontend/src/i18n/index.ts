/**
 * Traduzioni dell'interfaccia.
 *
 * - Il testo originale (inglese) è la chiave (`msgid`, come in gettext).
 * - Le traduzioni stanno nei file `src/i18n/locales/<lingua>.po`.
 * - Lingua predefinita: italiano. La scelta dell'utente è salvata in localStorage.
 * - Cambiare lingua ricarica la finestra, così ogni testo si aggiorna in modo coerente.
 *
 * Uso: `import { t } from '@/i18n'` e poi `t('Save')` oppure `t('Hello {name}', { name })`.
 *
 * Per aggiungere una lingua: copia `messages.pot` in `locales/<codice>.po`, traducilo (Poedit va bene)
 * e registralo in `LOCALES` qui sotto.
 */
import itPo from './locales/it.po';
import enPo from './locales/en.po';
import { parsePo, type Catalog } from './po';

export interface LocaleInfo {
  code: string;
  /** Nome della lingua scritto nella lingua stessa */
  name: string;
  source: string;
}

export const DEFAULT_LOCALE = 'it';
const STORAGE_KEY = 'trascrivi-seduta.locale';

export const LOCALES: LocaleInfo[] = [
  { code: 'it', name: 'Italiano', source: itPo },
  { code: 'en', name: 'English', source: enPo },
];

const catalogs = new Map<string, Catalog>();

function catalogFor(code: string): Catalog {
  let catalog = catalogs.get(code);
  if (!catalog) {
    const info = LOCALES.find((l) => l.code === code);
    catalog = info ? parsePo(info.source) : new Map();
    catalogs.set(code, catalog);
  }
  return catalog;
}

function readStoredLocale(): string {
  try {
    if (typeof window !== 'undefined') {
      const stored = window.localStorage.getItem(STORAGE_KEY);
      if (stored && LOCALES.some((l) => l.code === stored)) return stored;
    }
  } catch {
    // localStorage non disponibile: si usa la lingua predefinita
  }
  return DEFAULT_LOCALE;
}

let currentLocale = readStoredLocale();

export function getLocale(): string {
  return currentLocale;
}

/** Salva la lingua scelta e ricarica la finestra per applicarla ovunque. */
export function setLocale(code: string): void {
  if (!LOCALES.some((l) => l.code === code)) return;
  try {
    window.localStorage.setItem(STORAGE_KEY, code);
  } catch {
    // ignorato: la scelta vale solo per questa sessione
  }
  currentLocale = code;
  window.location.reload();
}

/** Traduce un testo. I segnaposto `{nome}` sono sostituiti dai valori passati. */
export function t(msgid: string, vars?: Record<string, string | number>): string {
  const translated = catalogFor(currentLocale).get(msgid) ?? msgid;
  if (!vars) return translated;
  return translated.replace(/\{(\w+)\}/g, (match, key: string) =>
    key in vars ? String(vars[key]) : match,
  );
}
