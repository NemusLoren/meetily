'use client';

import React, { useState } from 'react';
import { Lock } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  appLockService,
  lockErrorMessage,
  MIN_PASSWORD_LENGTH,
} from '@/services/appLockService';

interface LockScreenProps {
  onUnlocked: () => void;
}

export function LockScreen({ onUnlocked }: LockScreenProps) {
  const [mode, setMode] = useState<'unlock' | 'reset'>('unlock');
  const [password, setPassword] = useState('');
  const [question, setQuestion] = useState<string | null>(null);
  const [answer, setAnswer] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const handleUnlock = async () => {
    setBusy(true);
    setError(null);
    try {
      const ok = await appLockService.verify(password);
      if (ok) onUnlocked();
      else setError('Password errata. Dopo alcuni errori l’attesa si allunga.');
    } catch (e) {
      setError(lockErrorMessage(e));
    } finally {
      setBusy(false);
      setPassword('');
    }
  };

  const openReset = async () => {
    setError(null);
    try {
      setQuestion(await appLockService.question());
      setMode('reset');
    } catch (e) {
      setError(lockErrorMessage(e));
    }
  };

  const handleReset = async () => {
    if (newPassword.length < MIN_PASSWORD_LENGTH) {
      setError(`La nuova password deve avere almeno ${MIN_PASSWORD_LENGTH} caratteri.`);
      return;
    }
    if (newPassword !== confirm) {
      setError('Le due password non coincidono.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const ok = await appLockService.reset(answer, newPassword);
      if (ok) onUnlocked();
      else setError('Risposta errata.');
    } catch (e) {
      setError(lockErrorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-gray-50 px-6">
      <div className="w-full max-w-sm space-y-4 rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
        <div className="flex flex-col items-center gap-2 text-center">
          <div className="flex h-10 w-10 items-center justify-center rounded-full bg-gray-100">
            <Lock className="h-5 w-5 text-gray-700" aria-hidden="true" />
          </div>
          <h1 className="text-lg font-semibold text-gray-900">Trascrivi Seduta</h1>
          <p className="text-sm text-gray-600">
            {mode === 'unlock'
              ? 'Inserisci la password master per aprire l’app.'
              : 'Rispondi alla domanda segreta e scegli una nuova password. I tuoi dati non vengono toccati.'}
          </p>
        </div>

        {mode === 'unlock' ? (
          <form
            className="space-y-3"
            onSubmit={(e) => {
              e.preventDefault();
              void handleUnlock();
            }}
          >
            <Input
              type="password"
              autoFocus
              autoComplete="current-password"
              placeholder="Password master"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              aria-label="Password master"
            />
            {error && (
              <p role="alert" className="text-sm text-red-600">
                {error}
              </p>
            )}
            <Button
              type="submit"
              disabled={busy || password.length === 0}
              className="h-10 w-full bg-gray-900 text-white hover:bg-gray-800"
            >
              {busy ? 'Verifica…' : 'Apri'}
            </Button>
            <button
              type="button"
              onClick={() => void openReset()}
              className="w-full text-sm text-gray-600 underline underline-offset-2 hover:text-gray-900"
            >
              Password dimenticata?
            </button>
          </form>
        ) : (
          <form
            className="space-y-3"
            onSubmit={(e) => {
              e.preventDefault();
              void handleReset();
            }}
          >
            <p className="text-sm font-medium text-gray-800">{question ?? ''}</p>
            <Input
              type="password"
              autoFocus
              autoComplete="off"
              placeholder="Risposta"
              value={answer}
              onChange={(e) => setAnswer(e.target.value)}
              aria-label="Risposta alla domanda segreta"
            />
            <Input
              type="password"
              autoComplete="new-password"
              placeholder="Nuova password"
              value={newPassword}
              onChange={(e) => setNewPassword(e.target.value)}
              aria-label="Nuova password"
            />
            <Input
              type="password"
              autoComplete="new-password"
              placeholder="Ripeti la nuova password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              aria-label="Ripeti la nuova password"
            />
            {error && (
              <p role="alert" className="text-sm text-red-600">
                {error}
              </p>
            )}
            <Button
              type="submit"
              disabled={busy}
              className="h-10 w-full bg-gray-900 text-white hover:bg-gray-800"
            >
              {busy ? 'Verifica…' : 'Reimposta e apri'}
            </Button>
            <button
              type="button"
              onClick={() => {
                setMode('unlock');
                setError(null);
              }}
              className="w-full text-sm text-gray-600 underline underline-offset-2 hover:text-gray-900"
            >
              Torna indietro
            </button>
          </form>
        )}
      </div>
    </div>
  );
}
