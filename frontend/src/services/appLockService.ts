import { invoke } from '@tauri-apps/api/core';
import { t } from '@/i18n';

export interface AppLockStatus {
  configured: boolean;
  unlocked: boolean;
}

/** Lunghezza minima della password master (deve coincidere con il controllo in Rust). */
export const MIN_PASSWORD_LENGTH = 8;

/** Messaggio leggibile da un errore restituito da Tauri. */
export function lockErrorMessage(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  return t('Operation failed. Please try again.');
}

export const appLockService = {
  status: () => invoke<AppLockStatus>('app_lock_status'),
  question: () => invoke<string | null>('app_lock_question'),
  setup: (password: string, question: string, answer: string) =>
    invoke<void>('app_lock_setup', { password, question, answer }),
  verify: (password: string) => invoke<boolean>('app_lock_verify', { password }),
  reset: (answer: string, newPassword: string) =>
    invoke<boolean>('app_lock_reset', { answer, newPassword }),
};
