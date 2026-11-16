'use client';

import React, { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Archive, ArchiveRestore, ShieldCheck } from 'lucide-react';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { appLockService, lockErrorMessage, MIN_PASSWORD_LENGTH } from '@/services/appLockService';
import { t } from '@/i18n';

interface MeetingInfo {
  id: string;
  title: string;
  created_at: string;
  audio_files: number;
  audio_bytes: number;
  patient_id: string | null;
  session_date: string | null;
}

interface PatientInfo {
  id: string;
  name: string;
  protected: boolean;
  sessionCount: number;
}

/** Voce virtuale per le sedute senza paziente. */
const NO_PATIENT = '__none__';

interface Include {
  transcripts: boolean;
  summaries: boolean;
  notes: boolean;
  audio: boolean;
}

interface BackupResult {
  path: string;
  meetings: number;
  audioFiles: number;
  bytes: number;
}

interface Manifest {
  created_at: string;
  include: Include;
  meetings: { id: string; title: string; created_at: string; audio_files: number; patient_id?: string | null; session_date?: string | null }[];
  patients?: { id: string; name: string; protected: boolean }[];
}

interface RestoreResult {
  restored: number;
  skippedExisting: number;
  audioFiles: number;
  patientsRestored: number;
}

const formatBytes = (n: number): string => {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
};

const formatDate = (iso: string): string => {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString();
};

const errorText = (error: unknown): string => {
  const raw = lockErrorMessage(error);
  if (raw === 'WRONG_PASSWORD') return t("Wrong password.");
  if (raw === 'MASTER_PASSWORD_REQUIRED') return t("A master password is required to create backups.");
  return raw;
};

function useBackupProgress(active: boolean) {
  const [text, setText] = useState('');
  useEffect(() => {
    if (!active) {
      setText('');
      return;
    }
    let unlisten: (() => void) | undefined;
    listen<{ stage: string; done: number; total: number }>('backup-progress', (event) => {
      const { stage, done, total } = event.payload;
      const labels: Record<string, string> = {
        preparing: t("Preparing..."),
        database: t("Collecting sessions and transcripts..."),
        audio: total > 0 ? t("Adding audio {done} of {total}...", { done: done + 1, total }) : t("Restoring audio {done}...", { done }),
        finalizing: t("Encrypting and saving..."),
        done: t("Done"),
      };
      setText(labels[stage] ?? '');
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, [active]);
  return text;
}

function SessionList({
  meetings,
  selected,
  onChange,
  showAudio,
}: {
  meetings: { id: string; title: string; created_at: string; audio_bytes?: number }[];
  selected: Set<string>;
  onChange: (next: Set<string>) => void;
  showAudio: boolean;
}) {
  const [query, setQuery] = useState('');
  const shown = useMemo(
    () => meetings.filter((m) => m.title.toLowerCase().includes(query.trim().toLowerCase())),
    [meetings, query],
  );
  const toggle = (id: string) => {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    onChange(next);
  };
  return (
    <div className="mt-3 rounded-md border border-gray-200">
      <div className="flex items-center gap-2 border-b border-gray-200 p-2">
        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t("Search sessions...")}
          aria-label={t("Search sessions...")}
          className="h-8"
        />
        <button
          type="button"
          className="whitespace-nowrap text-sm text-blue-600 hover:underline"
          onClick={() => onChange(new Set(shown.map((m) => m.id).concat([...selected])))}
        >
          {t("Select all")}
        </button>
        <button
          type="button"
          className="whitespace-nowrap text-sm text-blue-600 hover:underline"
          onClick={() => onChange(new Set())}
        >
          {t("Select none")}
        </button>
      </div>
      <ul className="max-h-64 overflow-y-auto divide-y divide-gray-100">
        {shown.length === 0 && <li className="p-3 text-sm text-gray-500">{t("No sessions found.")}</li>}
        {shown.map((m) => (
          <li key={m.id}>
            <label className="flex cursor-pointer items-center gap-3 p-2 text-sm hover:bg-gray-50">
              <input type="checkbox" checked={selected.has(m.id)} onChange={() => toggle(m.id)} />
              <span className="flex-1 truncate">{m.title}</span>
              <span className="text-xs text-gray-500">{formatDate(m.created_at)}</span>
              {showAudio && typeof m.audio_bytes === 'number' && (
                <span className="w-20 text-right text-xs text-gray-500">{m.audio_bytes ? formatBytes(m.audio_bytes) : '—'}</span>
              )}
            </label>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Scelta dei pazienti (e di "Senza paziente") con ricerca. */
function PatientList({
  patients,
  counts,
  selected,
  onChange,
}: {
  patients: { id: string; name: string; protected?: boolean }[];
  /** Numero di sedute per paziente (chiave NO_PATIENT per le sedute senza paziente). */
  counts: Record<string, number>;
  selected: Set<string>;
  onChange: (next: Set<string>) => void;
}) {
  const [query, setQuery] = useState('');
  const rows = useMemo(() => {
    const all = [...patients.map((p) => ({ ...p })), { id: NO_PATIENT, name: t("No patient"), protected: false }];
    return all.filter((p) => p.name.toLowerCase().includes(query.trim().toLowerCase()));
  }, [patients, query]);
  const toggle = (id: string) => {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    onChange(next);
  };
  return (
    <div className="mt-3 rounded-md border border-gray-200">
      <div className="flex items-center gap-2 border-b border-gray-200 p-2">
        <Input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("Search patients...")} aria-label={t("Search patients...")} className="h-8" />
        <button type="button" className="whitespace-nowrap text-sm text-blue-600 hover:underline" onClick={() => onChange(new Set(rows.map((p) => p.id).concat([...selected])))}>
          {t("Select all")}
        </button>
        <button type="button" className="whitespace-nowrap text-sm text-blue-600 hover:underline" onClick={() => onChange(new Set())}>
          {t("Select none")}
        </button>
      </div>
      <ul className="max-h-64 divide-y divide-gray-100 overflow-y-auto">
        {rows.length === 0 && <li className="p-3 text-sm text-gray-500">{t("No patients found.")}</li>}
        {rows.map((p) => (
          <li key={p.id}>
            <label className="flex cursor-pointer items-center gap-3 p-2 text-sm hover:bg-gray-50">
              <input type="checkbox" checked={selected.has(p.id)} onChange={() => toggle(p.id)} />
              <span className="flex-1 truncate">{p.name}{p.protected ? ' 🔒' : ''}</span>
              <span className="text-xs text-gray-500">{t("{count} sessions", { count: counts[p.id] ?? 0 })}</span>
            </label>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Id delle sedute che appartengono ai pazienti scelti. */
function meetingsOfPatients(meetings: { id: string; patient_id?: string | null }[], chosen: Set<string>): string[] {
  return meetings
    .filter((m) => (m.patient_id ? chosen.has(m.patient_id) : chosen.has(NO_PATIENT)))
    .map((m) => m.id);
}

function countByPatient(meetings: { patient_id?: string | null }[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const m of meetings) {
    const key = m.patient_id ?? NO_PATIENT;
    counts[key] = (counts[key] ?? 0) + 1;
  }
  return counts;
}

function ScopeChooser({
  scope,
  setScope,
  total,
  idPrefix,
}: {
  scope: 'all' | 'patients' | 'some';
  setScope: (s: 'all' | 'patients' | 'some') => void;
  total: number;
  idPrefix: string;
}) {
  return (
    <div className="mt-3 space-y-2 text-sm text-gray-700">
      <label className="flex items-center gap-2">
        <input type="radio" name={`${idPrefix}-scope`} checked={scope === 'all'} onChange={() => setScope('all')} />
        {t("All patients and sessions ({count} sessions)", { count: total })}
      </label>
      <label className="flex items-center gap-2">
        <input type="radio" name={`${idPrefix}-scope`} checked={scope === 'patients'} onChange={() => setScope('patients')} />
        {t("Only some patients")}
      </label>
      <label className="flex items-center gap-2">
        <input type="radio" name={`${idPrefix}-scope`} checked={scope === 'some'} onChange={() => setScope('some')} />
        {t("Only some sessions")}
      </label>
    </div>
  );
}

function CreateBackup({ onBack }: { onBack: () => void }) {
  const [configured, setConfigured] = useState<boolean | null>(null);
  const [meetings, setMeetings] = useState<MeetingInfo[]>([]);
  const [include, setInclude] = useState<Include>({ transcripts: true, summaries: true, notes: true, audio: true });
  const [patients, setPatients] = useState<PatientInfo[]>([]);
  const [scope, setScope] = useState<'all' | 'patients' | 'some'>('all');
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [selectedPatients, setSelectedPatients] = useState<Set<string>>(new Set());
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<BackupResult | null>(null);
  const progress = useBackupProgress(busy);

  // creazione della password master, se manca
  const [newPassword, setNewPassword] = useState('');
  const [newConfirm, setNewConfirm] = useState('');
  const [question, setQuestion] = useState('');
  const [answer, setAnswer] = useState('');

  useEffect(() => {
    appLockService.status().then((s) => setConfigured(s.configured)).catch(() => setConfigured(false));
    invoke<MeetingInfo[]>('backup_list_meetings').then(setMeetings).catch((e) => setError(errorText(e)));
    invoke<PatientInfo[]>('backup_list_patients').then(setPatients).catch((e) => setError(errorText(e)));
  }, []);

  const audioTotal = useMemo(() => meetings.reduce((sum, m) => sum + m.audio_bytes, 0), [meetings]);
  const nothingSelected = !(include.transcripts || include.summaries || include.notes || include.audio);
  const noSessions = (scope === 'some' && selected.size === 0) || (scope === 'patients' && selectedPatients.size === 0);
  const patientCounts = useMemo(() => countByPatient(meetings), [meetings]);

  const createMasterPassword = async () => {
    setError(null);
    if (newPassword.length < MIN_PASSWORD_LENGTH) {
      setError(t("The password must have at least {MIN_PASSWORD_LENGTH} characters.", { MIN_PASSWORD_LENGTH }));
      return;
    }
    if (newPassword !== newConfirm) {
      setError(t("The two passwords do not match."));
      return;
    }
    setBusy(true);
    try {
      await appLockService.setup(newPassword, question, answer);
      setConfigured(true);
      setPassword(newPassword);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const run = async () => {
    setError(null);
    const stamp = new Date().toISOString().slice(0, 10);
    const destination = await invoke<string | null>('backup_pick_save_path', {
      suggestedName: `Trascrivi-Seduta-backup-${stamp}.tsbackup`,
    });
    if (!destination) return;
    setBusy(true);
    try {
      const res = await invoke<BackupResult>('backup_create', {
        request: {
          password,
          meetingIds:
            scope === 'all' ? null : scope === 'some' ? [...selected] : meetingsOfPatients(meetings, selectedPatients),
          patientIds: scope === 'patients' ? [...selectedPatients].filter((id) => id !== NO_PATIENT) : null,
          include,
          destination,
        },
      });
      setResult(res);
      setPassword('');
      toast.success(t("Backup created"));
    } catch (e) {
      const text = errorText(e);
      if (lockErrorMessage(e) === 'MASTER_PASSWORD_REQUIRED') setConfigured(false);
      setError(text);
    } finally {
      setBusy(false);
    }
  };

  if (result) {
    return (
      <div className="space-y-4">
        <div className="rounded-lg border border-green-200 bg-green-50 p-5 text-sm text-green-900">
          <p className="font-semibold">{t("Backup created")}</p>
          <p className="mt-2 break-all">{result.path}</p>
          <p className="mt-2">
            {t("{count} sessions, {audio} audio files, {size}", {
              count: result.meetings,
              audio: result.audioFiles,
              size: formatBytes(result.bytes),
            })}
          </p>
        </div>
        <div className="rounded-lg border border-amber-200 bg-amber-50 p-4 text-sm text-amber-900">
          {t("To open this backup you will need the master password you used today. Keep it in a safe place: without it the backup cannot be recovered.")}
        </div>
        <Button variant="outline" onClick={onBack}>{t("Done")}</Button>
      </div>
    );
  }

  return (
    <div className="space-y-5">
      <button type="button" onClick={onBack} className="text-sm text-gray-600 hover:text-gray-900">
        ← {t("Back")}
      </button>
      <h3 className="text-lg font-semibold text-gray-900">{t("Create encrypted backup")}</h3>

      {configured === false && (
        <div className="rounded-lg border border-gray-200 bg-white p-5 shadow-sm">
          <p className="text-sm text-gray-700">
            {t("A backup is always encrypted with your master password. Create one now to continue.")}
          </p>
          <div className="mt-3 max-w-md space-y-3">
            <Input type="password" autoComplete="new-password" placeholder={t("Master password")} value={newPassword} onChange={(e) => setNewPassword(e.target.value)} aria-label={t("Master password")} />
            <Input type="password" autoComplete="new-password" placeholder={t("Repeat the password")} value={newConfirm} onChange={(e) => setNewConfirm(e.target.value)} aria-label={t("Repeat the password")} />
            <Input type="text" placeholder={t("Secret question")} value={question} onChange={(e) => setQuestion(e.target.value)} aria-label={t("Secret question")} />
            <Input type="password" autoComplete="off" placeholder={t("Answer")} value={answer} onChange={(e) => setAnswer(e.target.value)} aria-label={t("Answer to the secret question")} />
            <Button onClick={() => void createMasterPassword()} disabled={busy}>{t("Set password")}</Button>
          </div>
        </div>
      )}

      {configured && (
        <>
          <div className="rounded-lg border border-gray-200 bg-white p-5 shadow-sm">
            <p className="text-sm font-semibold text-gray-900">{t("What to include")}</p>
            <div className="mt-3 space-y-2 text-sm text-gray-700">
              {([
                ['transcripts', t("Transcripts")],
                ['summaries', t("Summaries")],
                ['notes', t("Notes")],
                ['audio', `${t("Audio recordings")}${audioTotal ? ` (${formatBytes(audioTotal)})` : ''}`],
              ] as [keyof Include, string][]).map(([key, label]) => (
                <label key={key} className="flex items-center gap-2">
                  <input type="checkbox" checked={include[key]} onChange={(e) => setInclude({ ...include, [key]: e.target.checked })} />
                  {label}
                </label>
              ))}
            </div>
          </div>

          <div className="rounded-lg border border-gray-200 bg-white p-5 shadow-sm">
            <p className="text-sm font-semibold text-gray-900">{t("Which patients and sessions")}</p>
            <ScopeChooser scope={scope} setScope={setScope} total={meetings.length} idPrefix="create" />
            {scope === 'patients' && (
              <PatientList patients={patients} counts={patientCounts} selected={selectedPatients} onChange={setSelectedPatients} />
            )}
            {scope === 'some' && (
              <SessionList meetings={meetings} selected={selected} onChange={setSelected} showAudio={include.audio} />
            )}
          </div>

          <div className="rounded-lg border border-gray-200 bg-white p-5 shadow-sm">
            <p className="text-sm font-semibold text-gray-900">{t("Master password")}</p>
            <p className="mt-1 text-sm text-gray-600">{t("Enter your master password: it encrypts the backup.")}</p>
            <div className="mt-3 max-w-md">
              <Input type="password" autoComplete="current-password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder={t("Master password")} aria-label={t("Master password")} />
            </div>
          </div>

          <div className="rounded-lg border border-blue-100 bg-blue-50 p-4 text-sm text-blue-900">
            {t("The file can be saved anywhere (computer, USB drive, Dropbox, Google Drive...). It stays unreadable without your master password, and the app never connects to those services.")}
          </div>
        </>
      )}

      {error && <p role="alert" className="text-sm text-red-600">{error}</p>}
      {busy && progress && <p className="text-sm text-gray-600">{progress}</p>}

      {configured && (
        <Button
          onClick={() => void run()}
          disabled={busy || nothingSelected || noSessions || password.length === 0}
          className="bg-gray-900 text-white hover:bg-gray-800"
        >
          {busy ? t("Creating backup...") : t("Choose where to save and create backup")}
        </Button>
      )}
    </div>
  );
}

function RestoreBackup({ onBack }: { onBack: () => void }) {
  const [path, setPath] = useState<string | null>(null);
  const [password, setPassword] = useState('');
  const [manifest, setManifest] = useState<Manifest | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [scope, setScope] = useState<'all' | 'patients' | 'some'>('all');
  const [selectedPatients, setSelectedPatients] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<RestoreResult | null>(null);
  const progress = useBackupProgress(busy);

  const pick = async () => {
    const picked = await invoke<string | null>('backup_pick_open_path');
    if (picked) {
      setPath(picked);
      setManifest(null);
      setError(null);
    }
  };

  const open = async () => {
    if (!path) return;
    setBusy(true);
    setError(null);
    try {
      const m = await invoke<Manifest>('backup_inspect', { path, password });
      setManifest(m);
      setSelected(new Set(m.meetings.map((x) => x.id)));
      setSelectedPatients(new Set([...(m.patients ?? []).map((p) => p.id), NO_PATIENT]));
      setScope('all');
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const restore = async () => {
    if (!path || !manifest) return;
    setBusy(true);
    setError(null);
    try {
      const res = await invoke<RestoreResult>('backup_restore', {
        request: {
          path,
          password,
          meetingIds:
            scope === 'all' ? null : scope === 'some' ? [...selected] : meetingsOfPatients(manifest.meetings, selectedPatients),
          patientIds: scope === 'patients' ? [...selectedPatients].filter((id) => id !== NO_PATIENT) : null,
        },
      });
      setResult(res);
      setPassword('');
      toast.success(t("Backup restored"));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  if (result) {
    return (
      <div className="space-y-4">
        <div className="rounded-lg border border-green-200 bg-green-50 p-5 text-sm text-green-900">
          <p className="font-semibold">{t("Backup restored")}</p>
          <p className="mt-2">
            {t("{restored} sessions restored, {audio} audio files, {skipped} already present and left untouched.", {
              restored: result.restored,
              audio: result.audioFiles,
              skipped: result.skippedExisting,
            })}
            {' '}
            {t("{count} patient folders restored.", { count: result.patientsRestored })}
          </p>
        </div>
        <div className="flex gap-2">
          <Button onClick={() => window.location.reload()}>{t("Reload the app")}</Button>
          <Button variant="outline" onClick={onBack}>{t("Done")}</Button>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-5">
      <button type="button" onClick={onBack} className="text-sm text-gray-600 hover:text-gray-900">
        ← {t("Back")}
      </button>
      <h3 className="text-lg font-semibold text-gray-900">{t("Restore from a backup")}</h3>

      <div className="rounded-lg border border-gray-200 bg-white p-5 shadow-sm space-y-3">
        <div className="flex flex-wrap items-center gap-3">
          <Button variant="outline" onClick={() => void pick()}>{t("Choose backup file")}</Button>
          <span className="break-all text-sm text-gray-600">{path ?? t("No file chosen")}</span>
        </div>
        {path && !manifest && (
          <div className="max-w-md space-y-3">
            <p className="text-sm text-gray-600">{t("Enter the master password that was in use when the backup was made.")}</p>
            <Input type="password" autoComplete="off" value={password} onChange={(e) => setPassword(e.target.value)} placeholder={t("Master password")} aria-label={t("Master password")} />
            <Button onClick={() => void open()} disabled={busy || password.length === 0}>
              {busy ? t("Opening...") : t("Open backup")}
            </Button>
          </div>
        )}
      </div>

      {manifest && (
        <div className="rounded-lg border border-gray-200 bg-white p-5 shadow-sm">
          <p className="text-sm text-gray-700">
            {t("Backup created on {date}, with {count} sessions.", { date: formatDate(manifest.created_at), count: manifest.meetings.length })}
          </p>
          <p className="mt-1 text-sm text-gray-600">
            {t("Sessions already in the app are never overwritten.")}
          </p>
          <ScopeChooser scope={scope} setScope={setScope} total={manifest.meetings.length} idPrefix="restore" />
          {scope === 'patients' && (
            <PatientList
              patients={manifest.patients ?? []}
              counts={countByPatient(manifest.meetings)}
              selected={selectedPatients}
              onChange={setSelectedPatients}
            />
          )}
          {scope === 'some' && (
            <SessionList meetings={manifest.meetings} selected={selected} onChange={setSelected} showAudio={false} />
          )}
          <div className="mt-4">
            <Button onClick={() => void restore()} disabled={busy || (scope === 'some' && selected.size === 0) || (scope === 'patients' && selectedPatients.size === 0)} className="bg-gray-900 text-white hover:bg-gray-800">
              {busy
                ? t("Restoring...")
                : scope === 'all'
                  ? t("Restore {count} sessions", { count: manifest.meetings.length })
                  : scope === 'some'
                    ? t("Restore {count} sessions", { count: selected.size })
                    : t("Restore {count} sessions", { count: meetingsOfPatients(manifest.meetings, selectedPatients).length })}
            </Button>
          </div>
        </div>
      )}

      {error && <p role="alert" className="text-sm text-red-600">{error}</p>}
      {busy && progress && <p className="text-sm text-gray-600">{progress}</p>}
    </div>
  );
}

export function BackupSettings() {
  const [view, setView] = useState<'home' | 'create' | 'restore'>('home');

  if (view === 'create') return <CreateBackup onBack={() => setView('home')} />;
  if (view === 'restore') return <RestoreBackup onBack={() => setView('home')} />;

  return (
    <div className="space-y-5">
      <div className="rounded-lg border border-blue-100 bg-blue-50 p-4 text-sm text-blue-900">
        <p className="flex items-start gap-2">
          <ShieldCheck className="mt-0.5 h-4 w-4 flex-shrink-0" aria-hidden="true" />
          <span>
            {t("Backups are encrypted with your master password. Save them where you like: only someone who knows the password can open them.")}
          </span>
        </p>
      </div>
      <div className="grid gap-5 md:grid-cols-2">
        <button
          type="button"
          onClick={() => setView('create')}
          className="rounded-lg border border-gray-200 bg-white p-6 text-left shadow-sm transition-colors hover:border-gray-300 hover:bg-gray-50"
        >
          <Archive className="h-6 w-6 text-gray-700" aria-hidden="true" />
          <h3 className="mt-3 text-lg font-semibold text-gray-900">{t("Create encrypted backup")}</h3>
          <p className="mt-1 text-sm text-gray-600">{t("Choose what to include: everything, only some patients, only some sessions, or only some types of files.")}</p>
        </button>
        <button
          type="button"
          onClick={() => setView('restore')}
          className="rounded-lg border border-gray-200 bg-white p-6 text-left shadow-sm transition-colors hover:border-gray-300 hover:bg-gray-50"
        >
          <ArchiveRestore className="h-6 w-6 text-gray-700" aria-hidden="true" />
          <h3 className="mt-3 text-lg font-semibold text-gray-900">{t("Restore from a backup")}</h3>
          <p className="mt-1 text-sm text-gray-600">{t("Open a backup file and bring back the sessions you choose.")}</p>
        </button>
      </div>
    </div>
  );
}
