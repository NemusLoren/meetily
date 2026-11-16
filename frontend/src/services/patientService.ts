import { invoke } from '@tauri-apps/api/core';
import { t } from '@/i18n';

export interface SessionItem {
  id: string;
  title: string;
  createdAt: string;
}

export interface SessionDay {
  /** Giorno ISO AAAA-MM-GG */
  date: string;
  sessions: SessionItem[];
}

export interface PatientFolder {
  id: string;
  name: string;
  notes: string | null;
  archived: boolean;
  protected: boolean;
  /** Protetta e non ancora sbloccata in questa sessione: giorni e sedute non vengono restituiti. */
  locked: boolean;
  sessionCount: number;
  days: SessionDay[];
}

export interface PatientsTree {
  patients: PatientFolder[];
  /** Voce virtuale "Senza paziente" */
  unassigned: SessionDay[];
  unassignedCount: number;
}

/** Chiavi di sessionStorage con la scelta fatta prima della registrazione. */
export const RECORDING_PATIENT_KEY = 'recording_patient_id';
export const RECORDING_DATE_KEY = 'recording_session_date';
export const RECORDING_PATIENT_EVENT = 'recording-patient-changed';

/** Giorno locale di oggi in formato ISO AAAA-MM-GG. */
export function todayIso(): string {
  const d = new Date();
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  return `${d.getFullYear()}-${mm}-${dd}`;
}

/** Data ISO leggibile (es. "lunedì 9 marzo 2026"); se non valida restituisce il testo originale. */
export function formatDay(iso: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m) return iso;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString(undefined, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
}

export function readRecordingChoice(): { patientId: string | null; sessionDate: string } {
  try {
    return {
      patientId: sessionStorage.getItem(RECORDING_PATIENT_KEY) || null,
      sessionDate: sessionStorage.getItem(RECORDING_DATE_KEY) || todayIso(),
    };
  } catch {
    return { patientId: null, sessionDate: todayIso() };
  }
}

export function writeRecordingChoice(patientId: string | null, sessionDate: string | null) {
  try {
    if (patientId) sessionStorage.setItem(RECORDING_PATIENT_KEY, patientId);
    else sessionStorage.removeItem(RECORDING_PATIENT_KEY);
    if (sessionDate) sessionStorage.setItem(RECORDING_DATE_KEY, sessionDate);
    else sessionStorage.removeItem(RECORDING_DATE_KEY);
  } catch {
    // sessionStorage non disponibile: si salva senza paziente
  }
  window.dispatchEvent(new CustomEvent(RECORDING_PATIENT_EVENT));
}

/** Messaggio leggibile da un errore dei comandi pazienti. */
export function patientErrorMessage(error: unknown): string {
  const raw = typeof error === 'string' ? error : error instanceof Error ? error.message : '';
  if (raw === 'PATIENT_LOCKED') return t("This folder is locked. Unlock it with its password first.");
  if (raw === 'WRONG_PASSWORD') return t("Wrong password.");
  return raw || t('Operation failed. Please try again.');
}

export const MIN_PATIENT_PASSWORD_LENGTH = 8;

export const patientService = {
  list: (includeArchived = false) => invoke<PatientsTree>('patients_list', { includeArchived }),
  create: (name: string, notes?: string | null) =>
    invoke<{ id: string; name: string }>('patient_create', { name, notes: notes ?? null }),
  update: (id: string, name: string, notes?: string | null) =>
    invoke<void>('patient_update', { id, name, notes: notes ?? null }),
  setArchived: (id: string, archived: boolean) => invoke<void>('patient_set_archived', { id, archived }),
  remove: (id: string) => invoke<{ movedSessions: number }>('patient_delete', { id }),
  setPassword: (id: string, newPassword: string, currentPassword?: string | null) =>
    invoke<boolean>('patient_set_password', { id, newPassword, currentPassword: currentPassword ?? null }),
  removePassword: (id: string, password: string) => invoke<boolean>('patient_remove_password', { id, password }),
  unlock: (id: string, password: string) => invoke<boolean>('patient_unlock', { id, password }),
  lock: (id: string) => invoke<void>('patient_lock', { id }),
  assignMeeting: (meetingId: string, patientId: string | null, sessionDate?: string | null) =>
    invoke<void>('meeting_assign_patient', { meetingId, patientId, sessionDate: sessionDate ?? null }),
};
