'use client';

import { ArrowLeft, Copy, Mail } from 'lucide-react';
import { useRouter } from 'next/navigation';
import { toast } from 'sonner';

const FEEDBACK_EMAIL = 'info@leandromancino.it';

async function copyEmail() {
  try {
    await navigator.clipboard.writeText(FEEDBACK_EMAIL);
    toast.success('Indirizzo email copiato');
  } catch (error) {
    console.error('Failed to copy email address:', error);
    toast.error(`Impossibile copiare. Scrivi a ${FEEDBACK_EMAIL}`);
  }
}

export default function FeedbackPage() {
  const router = useRouter();

  return (
    <div className="h-screen overflow-y-auto bg-gray-50">
      <div className="mx-auto w-full max-w-2xl px-8 py-8">
        <button
          type="button"
          onClick={() => router.back()}
          className="flex items-center gap-2 rounded text-gray-600 transition-colors hover:text-gray-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2"
        >
          <ArrowLeft aria-hidden="true" className="h-4 w-4" />
          Indietro
        </button>
        <h1 className="mt-6 mb-8 text-2xl font-medium text-gray-900">Feedback e collaborazione</h1>

        <article className="rounded-lg border border-gray-200 bg-white p-6">
          <p className="text-sm leading-6 text-gray-700">
            Trascrivi Seduta nasce per i colleghi psicologi e psicoterapeuti e cresce grazie al
            loro contributo. Se hai trovato un problema, hai un&apos;idea per migliorarlo o vuoi
            semplicemente dirci come lo stai usando, ci farebbe molto piacere sentirti.
          </p>
          <p className="mt-3 text-sm leading-6 text-gray-700">
            Ti invitiamo con gentilezza a partecipare e a collaborare: ogni suggerimento,
            segnalazione o proposta di traduzione è prezioso.
          </p>

          <div className="mt-5 flex flex-wrap items-center gap-3">
            <span className="inline-flex items-center gap-2 text-sm font-semibold text-gray-900">
              <Mail aria-hidden="true" className="h-4 w-4" />
              <span className="select-text">{FEEDBACK_EMAIL}</span>
            </span>
            <button
              type="button"
              onClick={() => void copyEmail()}
              className="inline-flex min-h-11 items-center gap-1 rounded border border-gray-300 px-3 text-sm font-semibold text-gray-700 transition-colors hover:bg-gray-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2"
            >
              <Copy aria-hidden="true" className="h-4 w-4" />
              Copia indirizzo
            </button>
          </div>

          <p className="mt-5 text-xs leading-5 text-gray-500">
            Per la privacy dei tuoi pazienti, non allegare mai registrazioni, trascrizioni o dati
            identificativi nelle segnalazioni.
          </p>
        </article>
      </div>
    </div>
  );
}
