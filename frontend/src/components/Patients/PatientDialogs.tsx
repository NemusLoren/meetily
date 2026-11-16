'use client';

import React, { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogTitle } from '@/components/ui/dialog';
import {
  MIN_PATIENT_PASSWORD_LENGTH,
  patientErrorMessage,
  patientService,
  todayIso,
  type PatientFolder,
} from '@/services/patientService';
import { t } from '@/i18n';

function ErrorLine({ text }: { text: string | null }) {
  return text ? <p role="alert" className="text-sm text-red-600">{text}</p> : null;
}

/** Crea o rinomina un paziente (nome o sigla + note facoltative). */
export function PatientNameDialog({
  open,
  patient,
  onClose,
  onDone,
}: {
  open: boolean;
  /** Se presente: rinomina; altrimenti crea. */
  patient?: PatientFolder | null;
  onClose: () => void;
  onDone: (id: string) => void;
}) {
  const [name, setName] = useState('');
  const [notes, setNotes] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) {
      setName(patient?.name ?? '');
      setNotes(patient?.notes ?? '');
      setError(null);
    }
  }, [open, patient]);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      if (patient) {
        await patientService.update(patient.id, name, notes);
        onDone(patient.id);
      } else {
        const created = await patientService.create(name, notes);
        onDone(created.id);
      }
      onClose();
    } catch (e) {
      setError(patientErrorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-[425px]">
        <DialogTitle>{patient ? t("Rename patient") : t("New patient")}</DialogTitle>
        <DialogDescription>{t("Use the name or an initials code, as you prefer.")}</DialogDescription>
        <div className="space-y-3">
          <Input
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && void save()}
            placeholder={t("Name or initials")}
            aria-label={t("Name or initials")}
            maxLength={120}
          />
          <textarea
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            placeholder={t("Notes (optional)")}
            aria-label={t("Notes (optional)")}
            rows={3}
            maxLength={2000}
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
          <ErrorLine text={error} />
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>{t("Cancel")}</Button>
          <Button onClick={() => void save()} disabled={busy || !name.trim()}>{t("Save")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** Imposta, cambia o rimuove la password di una cartella (con doppia conferma della nuova password). */
export function PatientPasswordDialog({
  open,
  patient,
  onClose,
  onDone,
}: {
  open: boolean;
  patient: PatientFolder | null;
  onClose: () => void;
  onDone: () => void;
}) {
  const [current, setCurrent] = useState('');
  const [next, setNext] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) {
      setCurrent('');
      setNext('');
      setConfirm('');
      setError(null);
    }
  }, [open, patient?.id]);

  if (!patient) return null;
  const isProtected = patient.protected;

  const wrong = () => setError(t("Wrong password. After a few mistakes the wait gets longer."));

  const save = async () => {
    setError(null);
    if (next.length < MIN_PATIENT_PASSWORD_LENGTH) {
      setError(t("The password must have at least {MIN_PASSWORD_LENGTH} characters.", { MIN_PASSWORD_LENGTH: MIN_PATIENT_PASSWORD_LENGTH }));
      return;
    }
    if (next !== confirm) {
      setError(t("The two passwords do not match."));
      return;
    }
    setBusy(true);
    try {
      const ok = await patientService.setPassword(patient.id, next, isProtected ? current : null);
      if (!ok) return wrong();
      onDone();
      onClose();
    } catch (e) {
      setError(patientErrorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setError(null);
    setBusy(true);
    try {
      const ok = await patientService.removePassword(patient.id, current);
      if (!ok) return wrong();
      onDone();
      onClose();
    } catch (e) {
      setError(patientErrorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-[460px]">
        <DialogTitle>{isProtected ? t("Folder password") : t("Protect folder with a password")}</DialogTitle>
        <DialogDescription>
          {t("The password locks this folder in the app. It does not yet encrypt the files on disk. If you forget it, this folder cannot be opened: there is no recovery.")}
        </DialogDescription>
        <div className="space-y-3">
          {isProtected && (
            <Input type="password" autoComplete="off" value={current} onChange={(e) => setCurrent(e.target.value)} placeholder={t("Current password")} aria-label={t("Current password")} />
          )}
          <Input type="password" autoComplete="new-password" value={next} onChange={(e) => setNext(e.target.value)} placeholder={isProtected ? t("New password") : t("Password")} aria-label={isProtected ? t("New password") : t("Password")} />
          <Input type="password" autoComplete="new-password" value={confirm} onChange={(e) => setConfirm(e.target.value)} placeholder={t("Repeat the password")} aria-label={t("Repeat the password")} />
          <ErrorLine text={error} />
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>{t("Cancel")}</Button>
          {isProtected && (
            <Button variant="outline" onClick={() => void remove()} disabled={busy || !current} className="text-red-600">
              {t("Remove password")}
            </Button>
          )}
          <Button onClick={() => void save()} disabled={busy || !next || !confirm || (isProtected && !current)}>
            {isProtected ? t("Change password") : t("Set password")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** Sblocca una cartella protetta per la sessione corrente. */
export function PatientUnlockDialog({
  open,
  patient,
  onClose,
  onUnlocked,
}: {
  open: boolean;
  patient: PatientFolder | null;
  onClose: () => void;
  onUnlocked: (id: string) => void;
}) {
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) {
      setPassword('');
      setError(null);
    }
  }, [open, patient?.id]);

  if (!patient) return null;

  const unlock = async () => {
    setBusy(true);
    setError(null);
    try {
      const ok = await patientService.unlock(patient.id, password);
      if (ok) {
        onUnlocked(patient.id);
        onClose();
      } else {
        setError(t("Wrong password. After a few mistakes the wait gets longer."));
      }
    } catch (e) {
      setError(patientErrorMessage(e));
    } finally {
      setBusy(false);
      setPassword('');
    }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-[400px]">
        <DialogTitle>{t("Unlock folder")}</DialogTitle>
        <DialogDescription>{t("Enter the password of the folder \"{name}\".", { name: patient.name })}</DialogDescription>
        <div className="space-y-3">
          <Input
            autoFocus
            type="password"
            autoComplete="off"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && password && void unlock()}
            placeholder={t("Password")}
            aria-label={t("Password")}
          />
          <ErrorLine text={error} />
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>{t("Cancel")}</Button>
          <Button onClick={() => void unlock()} disabled={busy || !password}>{t("Unlock")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** Conferma di eliminazione: le sedute non vengono cancellate, tornano in "Senza paziente". */
export function PatientDeleteDialog({
  open,
  patient,
  onClose,
  onDone,
}: {
  open: boolean;
  patient: PatientFolder | null;
  onClose: () => void;
  onDone: () => void;
}) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) setError(null);
  }, [open]);

  if (!patient) return null;

  const remove = async () => {
    setBusy(true);
    setError(null);
    try {
      await patientService.remove(patient.id);
      onDone();
      onClose();
    } catch (e) {
      setError(patientErrorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-[440px]">
        <DialogTitle>{t("Delete patient folder")}</DialogTitle>
        <DialogDescription>
          {t("Delete the folder \"{name}\"? Its {count} sessions are NOT deleted: they move to \"No patient\". You can assign them to another patient later.", { name: patient.name, count: patient.sessionCount })}
        </DialogDescription>
        <ErrorLine text={error} />
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>{t("Cancel")}</Button>
          <Button onClick={() => void remove()} disabled={busy} className="bg-red-600 text-white hover:bg-red-700">
            {t("Delete folder")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** Sposta una seduta in un altro paziente (o in "Senza paziente") e, se serve, in un altro giorno. */
export function MoveSessionDialog({
  open,
  meetingId,
  currentPatientId,
  currentDate,
  patients,
  onClose,
  onDone,
}: {
  open: boolean;
  meetingId: string | null;
  currentPatientId: string | null;
  currentDate: string | null;
  patients: PatientFolder[];
  onClose: () => void;
  onDone: () => void;
}) {
  const [target, setTarget] = useState<string>('');
  const [date, setDate] = useState(todayIso());
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) {
      setTarget(currentPatientId ?? '');
      setDate(currentDate ?? todayIso());
      setError(null);
    }
  }, [open, currentPatientId, currentDate]);

  if (!meetingId) return null;

  const move = async () => {
    setBusy(true);
    setError(null);
    try {
      await patientService.assignMeeting(meetingId, target || null, date);
      onDone();
      onClose();
    } catch (e) {
      setError(patientErrorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-[425px]">
        <DialogTitle>{t("Move session")}</DialogTitle>
        <DialogDescription>{t("Choose the patient and the day of this session.")}</DialogDescription>
        <div className="space-y-3">
          <label className="block text-sm font-medium text-gray-700">
            {t("Patient")}
            <select
              value={target}
              onChange={(e) => setTarget(e.target.value)}
              className="mt-1 w-full rounded-md border border-gray-300 px-3 py-2 text-sm"
            >
              <option value="">{t("No patient")}</option>
              {patients.filter((p) => !p.locked && (!p.archived || p.id === currentPatientId)).map((p) => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
          </label>
          <label className="block text-sm font-medium text-gray-700">
            {t("Session day")}
            <input
              type="date"
              value={date}
              onChange={(e) => setDate(e.target.value)}
              className="mt-1 w-full rounded-md border border-gray-300 px-3 py-2 text-sm"
            />
          </label>
          <ErrorLine text={error} />
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>{t("Cancel")}</Button>
          <Button onClick={() => void move()} disabled={busy || !date}>{t("Move")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
