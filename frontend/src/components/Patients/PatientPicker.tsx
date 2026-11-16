'use client';

import React, { useEffect, useState } from 'react';
import { Plus, UserRound } from 'lucide-react';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import {
  RECORDING_PATIENT_EVENT,
  readRecordingChoice,
  todayIso,
  writeRecordingChoice,
  type PatientFolder,
} from '@/services/patientService';
import { PatientNameDialog, PatientUnlockDialog } from './PatientDialogs';
import { t } from '@/i18n';

/**
 * Sceglie paziente e giorno della seduta prima (o durante) la registrazione.
 * La scelta resta in sessionStorage e viene letta al salvataggio (useRecordingStop):
 * non tocca il flusso di registrazione né i percorsi audio.
 */
export function PatientPicker() {
  const { patientsTree, refetchPatients } = useSidebar();
  const [patientId, setPatientId] = useState<string | null>(null);
  const [date, setDate] = useState(todayIso());
  const [creating, setCreating] = useState(false);
  const [unlocking, setUnlocking] = useState<PatientFolder | null>(null);

  useEffect(() => {
    const sync = () => {
      const c = readRecordingChoice();
      setPatientId(c.patientId);
      setDate(c.sessionDate);
    };
    sync();
    window.addEventListener(RECORDING_PATIENT_EVENT, sync);
    return () => window.removeEventListener(RECORDING_PATIENT_EVENT, sync);
  }, []);

  const patients = (patientsTree?.patients ?? []).filter((p) => !p.archived);
  // una scelta che non esiste più (paziente eliminato o cartella richiusa) torna a "Senza paziente"
  const selected = patients.find((p) => p.id === patientId && !p.locked) ? patientId : null;

  const choose = (id: string | null) => {
    setPatientId(id);
    writeRecordingChoice(id, date);
  };

  const onSelect = (value: string) => {
    if (!value) return choose(null);
    const p = patients.find((x) => x.id === value);
    if (p?.locked) setUnlocking(p);
    else choose(value);
  };

  const onDate = (value: string) => {
    const next = value || todayIso();
    setDate(next);
    writeRecordingChoice(selected, next);
  };

  return (
    <div className="flex flex-wrap items-center justify-center gap-2 rounded-full bg-white px-4 py-2 text-sm shadow-md">
      <UserRound className="h-4 w-4 text-gray-500" aria-hidden="true" />
      <select
        value={selected ?? ''}
        onChange={(e) => onSelect(e.target.value)}
        aria-label={t("Patient")}
        className="max-w-[200px] rounded-md border border-gray-300 px-2 py-1 text-sm"
      >
        <option value="">{t("No patient")}</option>
        {patients.map((p) => (
          <option key={p.id} value={p.id}>{p.locked ? `🔒 ${p.name}` : p.name}</option>
        ))}
      </select>
      <button
        type="button"
        onClick={() => setCreating(true)}
        className="inline-flex items-center gap-1 rounded-md px-2 py-1 text-blue-700 hover:bg-blue-50"
      >
        <Plus className="h-3.5 w-3.5" />
        {t("New patient")}
      </button>
      <input
        type="date"
        value={date}
        onChange={(e) => onDate(e.target.value)}
        aria-label={t("Session day")}
        className="rounded-md border border-gray-300 px-2 py-1 text-sm"
      />
      <PatientNameDialog
        open={creating}
        onClose={() => setCreating(false)}
        onDone={(id) => {
          void refetchPatients().then(() => choose(id));
        }}
      />
      <PatientUnlockDialog
        open={!!unlocking}
        patient={unlocking}
        onClose={() => setUnlocking(null)}
        onUnlocked={(id) => {
          void refetchPatients().then(() => choose(id));
        }}
      />
    </div>
  );
}
