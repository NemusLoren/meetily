'use client';

import React, { useMemo, useState } from 'react';
import { useRouter } from 'next/navigation';
import { ArrowLeft, Search, UserPlus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { PatientTree } from '@/components/Patients/PatientTree';
import { t } from '@/i18n';

/** Pagina delle sedute: pazienti > giorni > sedute, con ricerca. */
export default function PatientsPage() {
  const router = useRouter();
  const {
    patientsTree,
    refetchPatients,
    showArchivedPatients,
    setShowArchivedPatients,
    searchTranscripts,
    searchResults,
    setCurrentMeeting,
    currentMeeting,
  } = useSidebar();
  const [query, setQuery] = useState('');
  const [newPatientOpen, setNewPatientOpen] = useState(false);
  const matchedIds = useMemo(() => new Set(searchResults.map((r: { id: string }) => r.id)), [searchResults]);

  const onSearch = (value: string) => {
    setQuery(value);
    void searchTranscripts(value);
  };

  return (
    <div className="mx-auto max-w-3xl p-6">
      <div className="mb-4 flex items-center gap-3">
        <button onClick={() => router.push('/')} className="rounded-md p-1 hover:bg-gray-100" aria-label={t("Back")}>
          <ArrowLeft className="h-5 w-5" />
        </button>
        <h1 className="flex-1 text-2xl font-semibold text-gray-900">{t("Patients and sessions")}</h1>
        <Button onClick={() => setNewPatientOpen(true)}>
          <UserPlus className="h-4 w-4" />
          {t("New patient")}
        </Button>
      </div>
      <div className="mb-3 flex items-center gap-4">
        <div className="relative flex-1">
          <Search className="pointer-events-none absolute left-2.5 top-2.5 h-4 w-4 text-gray-400" aria-hidden="true" />
          <Input
            value={query}
            onChange={(e) => onSearch(e.target.value)}
            placeholder={t("Search patients and sessions...")}
            aria-label={t("Search patients and sessions...")}
            className="pl-8"
          />
        </div>
        <label className="flex items-center gap-2 text-sm text-gray-600">
          <input type="checkbox" checked={showArchivedPatients} onChange={(e) => setShowArchivedPatients(e.target.checked)} />
          {t("Show archived")}
        </label>
      </div>
      <p className="mb-3 text-xs text-gray-500">
        {t("Each patient has a folder, and inside it one subfolder per session day. A folder can have its own password: it locks the folder in the app, it does not yet encrypt files on disk.")}
      </p>
      <div className="rounded-lg border border-gray-200 bg-white p-2 shadow-sm">
        <PatientTree
          tree={patientsTree}
          query={query}
          matchedIds={matchedIds}
          currentMeetingId={currentMeeting?.id}
          onOpenSession={(id, title) => {
            setCurrentMeeting({ id, title });
            router.push(`/meeting-details?id=${id}`);
          }}
          onChanged={refetchPatients}
          newPatientOpen={newPatientOpen}
          onNewPatientClose={() => setNewPatientOpen(false)}
        />
      </div>
    </div>
  );
}
