/**
 * Statistiche d'uso: RIMOSSE.
 *
 * Trascrivi Seduta non raccoglie né invia alcun dato di utilizzo. Questo file resta solo come
 * "interruttore spento": ogni chiamata a `Analytics.xxx(...)` non fa nulla e non tocca né la rete
 * né il disco. Le chiamate sparse nel codice verranno eliminate durante la pulizia delle schermate.
 */

export interface AnalyticsProperties {
  [key: string]: string;
}

export interface DeviceInfo {
  platform: string;
  os_version: string;
  architecture: string;
}

export interface UserSession {
  session_id: string;
  user_id: string;
  start_time: string;
  last_heartbeat: string;
  is_active: boolean;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type NoopAnalytics = Record<string, (...args: any[]) => Promise<any>>;

export const Analytics: NoopAnalytics = new Proxy({} as NoopAnalytics, {
  get: () => async () => undefined,
});

export default Analytics;
