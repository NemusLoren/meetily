import React, { useEffect, useState } from 'react';
import { Eye, EyeOff, ShieldCheck } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { OnboardingContainer } from '../OnboardingContainer';
import {
  appLockService,
  lockErrorMessage,
  MIN_PASSWORD_LENGTH,
} from '@/services/appLockService';

interface SecurityStepProps {
  onDone: () => void;
}

export function SecurityStep({ onDone }: SecurityStepProps) {
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [question, setQuestion] = useState('');
  const [answer, setAnswer] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  // Se la password è già stata impostata (per esempio riavvio a metà wizard) si salta il passo.
  useEffect(() => {
    appLockService
      .status()
      .then((s) => {
        if (s.configured) onDone();
      })
      .catch(() => undefined);
  }, [onDone]);

  const validate = (): string | null => {
    if (password.length < MIN_PASSWORD_LENGTH) {
      return `La password deve avere almeno ${MIN_PASSWORD_LENGTH} caratteri.`;
    }
    if (password !== confirm) return 'Le due password non coincidono.';
    if (question.trim().length < 5) return 'Scrivi una domanda segreta (almeno 5 caratteri).';
    if (answer.trim().length < 3) return 'Scrivi la risposta alla domanda segreta.';
    return null;
  };

  const handleSave = async () => {
    const problem = validate();
    if (problem) {
      setError(problem);
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await appLockService.setup(password, question, answer);
      onDone();
    } catch (e) {
      setError(lockErrorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <OnboardingContainer
      title="Proteggi l'accesso"
      description="Scegli se bloccare l'app con una password master. È facoltativa."
      step={2}
      hideProgress={true}
    >
      <div className="flex flex-col items-center space-y-5 overflow-y-auto">
        <div className="w-full max-w-md rounded-lg border border-amber-200 bg-amber-50 p-4 text-sm text-amber-900">
          <p className="flex items-start gap-2">
            <ShieldCheck className="mt-0.5 h-4 w-4 flex-shrink-0" aria-hidden="true" />
            <span>
              La password <strong>blocca l&apos;apertura dell&apos;app</strong>. In questa versione
              i file di sedute e registrazioni <strong>non sono ancora cifrati</strong>: attiva anche
              FileVault (Mac) o BitLocker (Windows).
            </span>
          </p>
        </div>

        <form
          className="w-full max-w-md space-y-3"
          onSubmit={(e) => {
            e.preventDefault();
            void handleSave();
          }}
        >
          <div className="relative">
            <Input
              type={showPassword ? 'text' : 'password'}
              placeholder="Password master"
              autoComplete="new-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              aria-label="Password master"
            />
            <button
              type="button"
              onClick={() => setShowPassword((v) => !v)}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-800"
              aria-label={showPassword ? 'Nascondi password' : 'Mostra password'}
            >
              {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
            </button>
          </div>
          <Input
            type={showPassword ? 'text' : 'password'}
            placeholder="Ripeti la password"
            autoComplete="new-password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            aria-label="Ripeti la password"
          />

          <div className="pt-2">
            <p className="mb-2 text-xs text-gray-600">
              Domanda segreta per reimpostare la password se la dimentichi. Scegli una risposta che
              nessun altro possa indovinare.
            </p>
            <Input
              type="text"
              placeholder="Domanda segreta"
              value={question}
              onChange={(e) => setQuestion(e.target.value)}
              aria-label="Domanda segreta"
            />
            <Input
              type="password"
              className="mt-3"
              placeholder="Risposta"
              autoComplete="off"
              value={answer}
              onChange={(e) => setAnswer(e.target.value)}
              aria-label="Risposta alla domanda segreta"
            />
          </div>

          {error && (
            <p role="alert" className="text-sm text-red-600">
              {error}
            </p>
          )}

          <Button
            type="submit"
            disabled={saving}
            className="h-11 w-full bg-gray-900 text-white hover:bg-gray-800"
          >
            {saving ? 'Salvataggio…' : 'Imposta password'}
          </Button>
          <button
            type="button"
            onClick={onDone}
            disabled={saving}
            className="w-full text-sm text-gray-600 underline underline-offset-2 hover:text-gray-900"
          >
            Salta per ora
          </button>
          <p className="text-center text-xs text-gray-500">
            Senza password l&apos;app si apre direttamente. Se la dimentichi e perdi anche la risposta
            segreta, non potrai più sbloccare l&apos;app.
          </p>
        </form>
      </div>
    </OnboardingContainer>
  );
}
