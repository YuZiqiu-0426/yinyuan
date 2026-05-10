import type { Y2HttpInterceptorFn, Y2InternalRequest, Y2NextFn } from '@y2/shared';
import { isAxiosError } from '@y2/shared';
import { catchError, Observable, switchMap, take, throwError } from 'rxjs';
import type { AuthApiService } from './auth-api.service';
import type { WebSessionPayload } from './auth-api.types';
import { WEB_LOGIN_PATH, WEB_REFRESH_PATH } from './auth-api.types';
import type { AuthSessionService } from './auth-session.service';

function isAuthRefreshRequest(req: Y2InternalRequest): boolean {
  return req.method === 'POST' && req.url === WEB_REFRESH_PATH;
}

function isAuthLoginRequest(req: Y2InternalRequest): boolean {
  return req.method === 'POST' && req.url === WEB_LOGIN_PATH;
}

export function createBearerInterceptor(auth: AuthSessionService): Y2HttpInterceptorFn {
  return (req, next) => {
    if (isAuthLoginRequest(req)) {
      return next(req);
    }
    const token = auth.getAccessToken();
    if (!token) {
      return next(req);
    }
    const headers = { ...req.headers, ['Authorization']: `Bearer ${token}` };
    return next({ ...req, headers });
  };
}

export function createAuth401Interceptor(
  auth: AuthSessionService,
  getApi: () => AuthApiService,
): Y2HttpInterceptorFn {
  return (req, next) => run401Aware(req, next, auth, getApi, false);
}

function run401Aware(
  req: Y2InternalRequest,
  next: Y2NextFn,
  auth: AuthSessionService,
  getApi: () => AuthApiService,
  alreadyRetried: boolean,
): Observable<import('axios').AxiosResponse<unknown>> {
  return next(req).pipe(
    catchError((err: unknown) => {
      if (!isAxiosError(err) || err.response?.status !== 401) {
        return throwError(() => err);
      }
      if (isAuthRefreshRequest(req) || isAuthLoginRequest(req)) {
        return throwError(() => err);
      }
      if (alreadyRetried || !auth.getAccessToken()) {
        auth.logout();
        return throwError(() => err);
      }
      return getApi().refreshTokens().pipe(
        take(1),
        switchMap((payload) => {
          auth.applySessionPayload(payload as WebSessionPayload);
          const token = auth.getAccessToken();
          const headers = { ...req.headers };
          if (token) {
            headers['Authorization'] = `Bearer ${token}`;
          }
          return run401Aware({ ...req, headers }, next, auth, getApi, true);
        }),
        catchError(() => {
          auth.logout();
          return throwError(() => err);
        }),
      );
    }),
  );
}
