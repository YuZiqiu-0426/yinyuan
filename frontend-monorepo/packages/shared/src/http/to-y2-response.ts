import { AxiosHeaders, type AxiosResponse } from 'axios';
import type { Y2HttpResponse } from './types';

export function toY2HttpResponse<T>(res: AxiosResponse<T>): Y2HttpResponse<T> {
  const headers: Record<string, string> = {};
  const raw = res.headers;
  if (raw instanceof AxiosHeaders) {
    raw['forEach']((value: string, key: string) => {
      headers[key] = String(value);
    });
  } else if (raw && typeof raw === 'object') {
    for (const [k, v] of Object.entries(raw)) {
      headers[k] = v === undefined || v === null ? '' : String(v);
    }
  }
  return {
    status: res.status,
    statusText: res.statusText,
    data: res.data,
    headers,
  };
}
