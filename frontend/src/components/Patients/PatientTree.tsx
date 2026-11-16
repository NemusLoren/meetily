'use client';

import React, { useMemo, useState } from 'react';
import {
  Archive, ArchiveRestore, CalendarDays, ChevronDown, ChevronRight, File, FolderInput, KeyRound,
  Lock, LockOpen, Pencil, Trash2, User, UserRound,
} from 'lucide-react';
import { toast } from 'sonner';
import {
  formatDay,
  patientErrorMessage,
  patientService,
  type PatientFolder,
  type PatientsTree,
  type SessionDay,
} from '@/services/patientService';
import {
  MoveSessionDialog,
  PatientDeleteDialog,
  PatientNameDialog,
  PatientPasswordDialog,
  PatientUnlockDialog,
} from './PatientDialogs';
import { t } from '@/i18n';

const NONE = '__none__';

interface Props {
  tree: PatientsTree | null;
  /** Testo di ricerca (nome paziente, titolo seduta, giorno). */
  query?: string;
  /** Id di sedute trovate nel testo delle trascrizioni. */
  matchedIds?: Set<string>;
  compact?: boolean;
  currentMeetingId?: string | null;
  onOpenSession: (id: string, title: string) => void;
  onRenameSession?: (id: string, title: string) => void;
  onDeleteSession?: (id: string) => void;
  /** Dopo qualunque modifica (per ricaricare l'albero). */
  onChanged: () => void | Promise<void>;
  /** Apre la finestra "Nuovo paziente" (la crea il genitore o questo componente). */
  newPatientOpen?: boolean;
  onNewPatientClose?: () => void;
}

function filterDays(days: SessionDay[], q: string, matched?: Set<string>): SessionDay[] {
  if (!q) return days;
  return days
    .map((d) => {
      const dayMatches = d.date.includes(q) || formatDay(d.date).toLowerCase().includes(q);
      const sessions = dayMatches
        ? d.sessions
        : d.sessions.filter((s) => s.title.toLowerCase().includes(q) || matched?.has(s.id));
      return { ...d, sessions };
    })
    .filter((d) => d.sessions.length > 0);
}

export function PatientTree({
  tree,
  query = '',
  matchedIds,
  compact = false,
  currentMeetingId,
  onOpenSession,
  onRenameSession,
  onDeleteSession,
  onChanged,
  newPatientOpen = false,
  onNewPatientClose,
}: Props) {
  const [open, setOpen] = useState<Set<string>>(new Set());
  const [renaming, setRenaming] = useState<PatientFolder | null>(null);
  const [passwordFor, setPasswordFor] = useState<PatientFolder | null>(null);
  const [unlocking, setUnlocking] = useState<PatientFolder | null>(null);
  const [deleting, setDeleting] = useState<PatientFolder | null>(null);
  const [moving, setMoving] = useState<{ id: string; patientId: string | null; date: string } | null>(null);

  const q = query.trim().toLowerCase();
  const searching = q.length > 0;

  const view = useMemo(() => {
    if (!tree) return null;
    const patients = tree.patients
      .map((p) => {
        if (searching && p.name.toLowerCase().includes(q)) return { p, days: p.days };
        return { p, days: filterDays(p.days, q, matchedIds) };
      })
      .filter(({ p, days }) => !searching || p.name.toLowerCase().includes(q) || days.length > 0);
    const unassigned = filterDays(tree.unassigned, q, matchedIds);
    return { patients, unassigned };
  }, [tree, q, searching, matchedIds]);

  const toggle = (key: string) => {
    const next = new Set(open);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    setOpen(next);
  };
  const isOpen = (key: string) => searching || open.has(key);

  const guard = async (fn: () => Promise<unknown>, success?: string) => {
    try {
      await fn();
      if (success) toast.success(success);
      await onChanged();
    } catch (e) {
      toast.error(patientErrorMessage(e));
    }
  };

  const pad = compact ? 'px-2 py-1.5' : 'px-3 py-2';
  const iconBtn = 'p-1 rounded-md flex-shrink-0 hover:bg-gray-200 text-gray-500 hover:text-gray-800';

  const renderDays = (days: SessionDay[], patientId: string | null, keyPrefix: string) =>
    days.map((day) => {
      const dKey = `d:${keyPrefix}:${day.date}`;
      return (
        <div key={dKey}>
          <div
            className={`flex cursor-pointer items-center rounded-md ${pad} text-sm text-gray-700 hover:bg-gray-50`}
            style={{ paddingLeft: compact ? 22 : 30 }}
            onClick={() => toggle(dKey)}
          >
            {isOpen(dKey) ? <ChevronDown className="mr-1 h-3.5 w-3.5 text-gray-400" /> : <ChevronRight className="mr-1 h-3.5 w-3.5 text-gray-400" />}
            <CalendarDays className="mr-2 h-3.5 w-3.5 text-gray-500" />
            <span className="flex-1 truncate">{formatDay(day.date)}</span>
            <span className="text-xs text-gray-400">{day.sessions.length}</span>
          </div>
          {isOpen(dKey) &&
            day.sessions.map((s) => {
              const active = currentMeetingId === s.id;
              const hit = searching && matchedIds?.has(s.id);
              return (
                <div
                  key={s.id}
                  className={`group flex cursor-pointer items-center rounded-md ${pad} text-sm ${active ? 'bg-blue-100 font-medium text-blue-700' : hit ? 'bg-yellow-50' : 'hover:bg-gray-50'}`}
                  style={{ paddingLeft: compact ? 40 : 52 }}
                  onClick={() => onOpenSession(s.id, s.title)}
                >
                  <File className="mr-2 h-3.5 w-3.5 flex-shrink-0 text-gray-500" />
                  <span className="flex-1 break-words">{s.title}</span>
                  <div className="flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
                    <button
                      className={iconBtn}
                      aria-label={t("Move session")}
                      title={t("Move session")}
                      onClick={(e) => {
                        e.stopPropagation();
                        setMoving({ id: s.id, patientId, date: day.date });
                      }}
                    >
                      <FolderInput className="h-4 w-4" />
                    </button>
                    {onRenameSession && (
                      <button className={iconBtn} aria-label={t("Edit meeting title")} onClick={(e) => { e.stopPropagation(); onRenameSession(s.id, s.title); }}>
                        <Pencil className="h-4 w-4" />
                      </button>
                    )}
                    {onDeleteSession && (
                      <button className={`${iconBtn} hover:text-red-600`} aria-label={t("Delete meeting")} onClick={(e) => { e.stopPropagation(); onDeleteSession(s.id); }}>
                        <Trash2 className="h-4 w-4" />
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
        </div>
      );
    });

  const renderPatient = (p: PatientFolder, days: SessionDay[]) => {
    const key = `p:${p.id}`;
    const expanded = isOpen(key);
    return (
      <div key={p.id}>
        <div
          className={`group flex cursor-pointer items-center rounded-md ${pad} text-sm font-medium text-gray-800 hover:bg-gray-50 ${p.archived ? 'opacity-60' : ''}`}
          onClick={() => (p.locked ? setUnlocking(p) : toggle(key))}
        >
          {p.locked ? (
            <Lock className="mr-1 h-3.5 w-3.5 text-amber-600" aria-label={t("Locked folder")} />
          ) : expanded ? (
            <ChevronDown className="mr-1 h-3.5 w-3.5 text-gray-400" />
          ) : (
            <ChevronRight className="mr-1 h-3.5 w-3.5 text-gray-400" />
          )}
          <User className="mr-2 h-4 w-4 text-gray-600" />
          <span className="flex-1 truncate" title={p.name}>{p.name}{p.archived ? ` (${t("archived")})` : ''}</span>
          <span className="mr-1 text-xs font-normal text-gray-400">{p.sessionCount}</span>
          <div className="flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
            {!p.locked && (
              <>
                <button className={iconBtn} aria-label={t("Rename patient")} title={t("Rename patient")} onClick={(e) => { e.stopPropagation(); setRenaming(p); }}>
                  <Pencil className="h-4 w-4" />
                </button>
                <button className={iconBtn} aria-label={t("Folder password")} title={t("Folder password")} onClick={(e) => { e.stopPropagation(); setPasswordFor(p); }}>
                  <KeyRound className="h-4 w-4" />
                </button>
                {p.protected && (
                  <button
                    className={iconBtn}
                    aria-label={t("Lock folder now")}
                    title={t("Lock folder now")}
                    onClick={(e) => { e.stopPropagation(); void guard(() => patientService.lock(p.id)); }}
                  >
                    <LockOpen className="h-4 w-4" />
                  </button>
                )}
                <button
                  className={iconBtn}
                  aria-label={p.archived ? t("Restore patient") : t("Archive patient")}
                  title={p.archived ? t("Restore patient") : t("Archive patient")}
                  onClick={(e) => { e.stopPropagation(); void guard(() => patientService.setArchived(p.id, !p.archived)); }}
                >
                  {p.archived ? <ArchiveRestore className="h-4 w-4" /> : <Archive className="h-4 w-4" />}
                </button>
                <button className={`${iconBtn} hover:text-red-600`} aria-label={t("Delete patient folder")} title={t("Delete patient folder")} onClick={(e) => { e.stopPropagation(); setDeleting(p); }}>
                  <Trash2 className="h-4 w-4" />
                </button>
              </>
            )}
          </div>
        </div>
        {!p.locked && expanded && (
          <div>
            {p.notes && <p className="px-6 py-1 text-xs text-gray-500">{p.notes}</p>}
            {days.length === 0 && <p className="px-8 py-1 text-xs text-gray-400">{t("No sessions yet.")}</p>}
            {renderDays(days, p.id, p.id)}
          </div>
        )}
      </div>
    );
  };

  const allPatients = tree?.patients ?? [];

  return (
    <div className="text-sm">
      {view?.patients.map(({ p, days }) => renderPatient(p, days))}

      {tree && (tree.unassignedCount > 0 || view!.unassigned.length > 0) && (view!.unassigned.length > 0 || !searching) && (
        <div>
          <div className={`flex cursor-pointer items-center rounded-md ${pad} font-medium text-gray-600 hover:bg-gray-50`} onClick={() => toggle(`p:${NONE}`)}>
            {isOpen(`p:${NONE}`) ? <ChevronDown className="mr-1 h-3.5 w-3.5 text-gray-400" /> : <ChevronRight className="mr-1 h-3.5 w-3.5 text-gray-400" />}
            <UserRound className="mr-2 h-4 w-4 text-gray-500" />
            <span className="flex-1 truncate">{t("No patient")}</span>
            <span className="text-xs font-normal text-gray-400">{tree.unassignedCount}</span>
          </div>
          {isOpen(`p:${NONE}`) && renderDays(view!.unassigned, null, NONE)}
        </div>
      )}

      {tree && view!.patients.length === 0 && tree.unassignedCount === 0 && (
        <p className="p-3 text-xs text-gray-500">{searching ? t("No sessions found.") : t("No patients or sessions yet.")}</p>
      )}

      <PatientNameDialog
        open={newPatientOpen || !!renaming}
        patient={renaming}
        onClose={() => {
          setRenaming(null);
          onNewPatientClose?.();
        }}
        onDone={(id) => {
          void onChanged();
          setOpen((prev) => new Set(prev).add(`p:${id}`));
        }}
      />
      <PatientPasswordDialog open={!!passwordFor} patient={passwordFor} onClose={() => setPasswordFor(null)} onDone={() => { toast.success(t("Folder password updated")); void onChanged(); }} />
      <PatientUnlockDialog
        open={!!unlocking}
        patient={unlocking}
        onClose={() => setUnlocking(null)}
        onUnlocked={(id) => {
          setOpen((prev) => new Set(prev).add(`p:${id}`));
          void onChanged();
        }}
      />
      <PatientDeleteDialog open={!!deleting} patient={deleting} onClose={() => setDeleting(null)} onDone={() => { toast.success(t("Folder deleted. Its sessions are now under \"No patient\".")); void onChanged(); }} />
      <MoveSessionDialog
        open={!!moving}
        meetingId={moving?.id ?? null}
        currentPatientId={moving?.patientId ?? null}
        currentDate={moving?.date ?? null}
        patients={allPatients}
        onClose={() => setMoving(null)}
        onDone={() => { toast.success(t("Session moved")); void onChanged(); }}
      />
    </div>
  );
}
