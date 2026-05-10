import axios from 'axios';
import type { Y2HttpInterceptorFn } from '@y2/shared';
import { Y2HttpClient } from '@y2/shared';
import type { AppEnvironment } from '../../environments/environment.types';

export function createY2HttpClient(
  env: Pick<AppEnvironment, 'apiBaseUrl'>,
  interceptors: readonly Y2HttpInterceptorFn[],
): Y2HttpClient {
  const trimmed = env.apiBaseUrl.trim();
  const baseURL = trimmed.length > 0 ? trimmed : undefined;
  return new Y2HttpClient(axios.create({ baseURL }), interceptors);
}
