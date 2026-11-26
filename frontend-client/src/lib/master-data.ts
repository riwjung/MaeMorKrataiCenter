import {useSyncExternalStore} from 'react';
import readingCards from './reading-cards.json';
import dailyCards from './daily-cards.json';
import artManifest from '../../public/cards/v3/manifest.json';
import {categories, contexts, positions, situations, suggestions} from './reading-config';
import {adaptApiMasterData, fromBundled, type MasterData} from './master-data-adapter';

export type {MasterData};

// The bundled JSON renders the first paint and stays in use whenever the API is not configured or fails.
export const bundledMasterData: MasterData = fromBundled({
  readingCards, dailyCards, artManifest,
  taxonomy: {categories, contexts, positions, situations, suggestions},
});

// Which client API a hostname reads master data from (backend SPEC-002a). Staging only for now: production
// keeps the bundled data until the production API is deployed (backend D11, D14). The staging API allows
// exactly these two origins (backend/client/wrangler.toml ALLOWED_ORIGINS).
const STAGING_API = 'https://maemorkratai-backend-client-staging.maemorkratai.workers.dev';
const API_BY_HOST: Record<string, string> = {
  'mmk-frontend-client.maemorkratai.workers.dev': STAGING_API,
  'staging.maemorkratai.com': STAGING_API,
};
const TIMEOUT_MS = 8000;

// NEXT_PUBLIC_MASTER_DATA_API (read at build time) overrides the map, for local testing against `make dev-client`.
export function apiBaseFor(hostname: string): string | null {
  const override = process.env.NEXT_PUBLIC_MASTER_DATA_API;
  if (override) return override.replace(/\/+$/, '');
  return API_BY_HOST[hostname] ?? null;
}

let current = bundledMasterData;
const listeners = new Set<() => void>();
function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

export function getMasterData(): MasterData {
  return current;
}

export function useMasterData(): MasterData {
  return useSyncExternalStore(subscribe, getMasterData, () => bundledMasterData);
}

let started = false;

// Fetches all four master-data endpoints once per page load. The responses carry
// `Cache-Control: public, max-age=600`, so the browser reuses them for 10 minutes.
export function loadMasterData(): void {
  if (started || typeof window === 'undefined') return;
  started = true;
  const base = apiBaseFor(window.location.hostname);
  if (!base) return mark('bundled');
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), TIMEOUT_MS);
  const get = (path: string) => fetch(base + path, {signal: controller.signal, credentials: 'omit'})
    .then(res => { if (!res.ok) throw new Error(`${path} answered ${res.status}`); return res.json(); });
  Promise.all([get('/v1/cards'), get('/v1/daily-cards'), get('/v1/card-art'), get('/v1/reading-config')])
    .then(([cards, daily, art, config]) => {
      current = adaptApiMasterData({cards, daily, art, config});
      listeners.forEach(listener => listener());
      mark('api');
    })
    .catch(error => {
      console.warn('Master data API unavailable; showing the bundled data.', error);
      mark('bundled');
    })
    .finally(() => clearTimeout(timer));
}

// Visible in the DOM (<html data-master-data="api|bundled">) so a check can tell which data a page shows.
function mark(source: MasterData['source']) {
  document.documentElement.dataset.masterData = source;
}
