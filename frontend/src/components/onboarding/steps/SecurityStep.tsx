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

import { t } from '@/i18n';
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
      return t("The password must have at least {MIN_PASSWORD_LENGTH} characters.", { MIN_PASSWORD_LENGTH });
    }
    if (password !== confirm) return t("The two passwords do not match.");
    if (question.trim().length < 5) return t("Write a secret question (at least 5 characters).");
    if (answer.trim().length < 3) return t("Write the answer to the secret question.");
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
      title={t("Protect access")}
      description={t("Choose whether to lock the app with a master password. It is optional.")}
      step={2}
      hideProgress={true}
    >
      <div className="flex flex-col items-center space-y-5 overflow-y-auto">
        <div className="w-full max-w-md rounded-lg border border-amber-200 bg-amber-50 p-4 text-sm text-amber-900">
          <p className="flex items-start gap-2">
            <ShieldCheck className="mt-0.5 h-4 w-4 flex-shrink-0" aria-hidden="true" />
            <span>{t("The password")} <strong>{t("blocks opening the app")}</strong>{t(". In this version the session and recording files")} <strong>{t("are not encrypted yet")}</strong>{t(": also turn on FileVault (Mac) or BitLocker (Windows).")}
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
              placeholder={t("Master password")}
              autoComplete="new-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              aria-label={t("Master password")}
            />
            <button
              type="button"
              onClick={() => setShowPassword((v) => !v)}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-800"
              aria-label={showPassword ? t("Hide password") : t("Show password")}
            >
              {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
            </button>
          </div>
          <Input
            type={showPassword ? 'text' : 'password'}
            placeholder={t("Repeat the password")}
            autoComplete="new-password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            aria-label={t("Repeat the password")}
          />

          <div className="pt-2">
            <p className="mb-2 text-xs text-gray-600">{t("Secret question to reset the password if you forget it. Choose an answer nobody else can guess.")}
            </p>
            <Input
              type="text"
              placeholder={t("Secret question")}
              value={question}
              onChange={(e) => setQuestion(e.target.value)}
              aria-label={t("Secret question")}
            />
            <Input
              type="password"
              className="mt-3"
              placeholder={t("Answer")}
              autoComplete="off"
              value={answer}
              onChange={(e) => setAnswer(e.target.value)}
              aria-label={t("Answer to the secret question")}
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
            {saving ? t("Saving...") : t("Set password")}
          </Button>
          <button
            type="button"
            onClick={onDone}
            disabled={saving}
            className="w-full text-sm text-gray-600 underline underline-offset-2 hover:text-gray-900"
          >{t("Skip for now")}
          </button>
          <p className="text-center text-xs text-gray-500">{t("Without a password the app opens directly. If you forget it and also lose the secret answer, you will no longer be able to unlock the app.")}
          </p>
        </form>
      </div>
    </OnboardingContainer>
  );
}
