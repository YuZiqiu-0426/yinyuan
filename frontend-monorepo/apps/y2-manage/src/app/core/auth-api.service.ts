import { Injectable, inject } from '@angular/core';
import axios, { AxiosError } from 'axios';
import { Observable, firstValueFrom, from, map, of, switchMap, take, throwError, timer } from 'rxjs';
import { isAxiosError } from '@y2/shared';
import type { Y2HttpClient } from '@y2/shared';
import { environment } from '../../environments/environment';
import type {
  ApiEnvelope,
  MfaRequiredPayload,
  WebLoginRequestBody,
  WebLoginResult,
  WebMfaVerifyBody,
  WebSessionPayload,
} from './auth-api.types';
import { WEB_LOGIN_PATH, WEB_MFA_VERIFY_PATH, WEB_REFRESH_PATH } from './auth-api.types';
import { Y2_HTTP_CLIENT } from './tokens';

function unwrapOk<T>(en: ApiEnvelope<T>): T {
  if (en.code !== 'OK' || en.data === undefined) {
    const err = new Error(en.message ?? String(en.code));
    (err as Error & { authEnvelope?: ApiEnvelope<T> }).authEnvelope = en;
    throw err;
  }
  return en.data;
}

function parseLoginEnvelope(en: ApiEnvelope<unknown>): WebLoginResult {
  if (en.code === 'AUTH_MFA_REQUIRED') {
    const d = en.data as Partial<MfaRequiredPayload> | undefined;
    if (
      d &&
      typeof d.mfaTicket === 'string' &&
      typeof d.expiresInSeconds === 'number'
    ) {
      return {
        kind: 'mfa',
        payload: { mfaTicket: d.mfaTicket, expiresInSeconds: d.expiresInSeconds },
      };
    }
    const err = new Error('MFA 响应字段不完整');
    (err as Error & { authEnvelope?: ApiEnvelope<unknown> }).authEnvelope = en;
    throw err;
  }
  if (en.code === 'OK' && en.data !== undefined) {
    return { kind: 'session', payload: en.data as WebSessionPayload };
  }
  const err = new Error(en.message ?? String(en.code));
  (err as Error & { authEnvelope?: ApiEnvelope<unknown> }).authEnvelope = en;
  throw err;
}

@Injectable({ providedIn: 'root' })
export class AuthApiService {
  private readonly http = inject(Y2_HTTP_CLIENT) as Y2HttpClient;
  private readonly raw = axios.create({
    baseURL: this.baseUrl(),
    withCredentials: !environment.useAuthMock,
  });
  private refreshSingleton: Promise<WebSessionPayload> | null = null;

  login(body: WebLoginRequestBody): Observable<WebLoginResult> {
    if (environment.useAuthMock) {
      return this.mockLogin(body);
    }
    return this.http.post<ApiEnvelope<unknown>>(WEB_LOGIN_PATH, body).pipe(
      map((en) => parseLoginEnvelope(en)),
    );
  }

  verifyMfa(body: WebMfaVerifyBody): Observable<WebSessionPayload> {
    if (environment.useAuthMock) {
      return this.mockVerifyMfa(body);
    }
    return this.http.post<ApiEnvelope<WebSessionPayload>>(WEB_MFA_VERIFY_PATH, body).pipe(
      map((en) => unwrapOk(en)),
    );
  }

  /**
   * 并发安全：多次 401 同时触发时合并为一次 refresh，共享同一 Promise 结果。
   */
  refreshTokens(): Observable<WebSessionPayload> {
    if (!this.refreshSingleton) {
      this.refreshSingleton = firstValueFrom(this.executeRefreshOnce()).finally(() => {
        this.refreshSingleton = null;
      });
    }
    return from(this.refreshSingleton);
  }

  private executeRefreshOnce(): Observable<WebSessionPayload> {
    if (environment.useAuthMock) {
      return timer(20).pipe(
        take(1),
        map(() => this.mockRefreshPayload()),
      );
    }
    const headers: Record<string, string> = {
      'X-CSRF-Token': environment.devCsrfToken,
    };
    return from(
      this.raw.post<ApiEnvelope<WebSessionPayload>>(WEB_REFRESH_PATH, null, { headers }),
    ).pipe(map((res) => unwrapOk(res.data)));
  }

  private baseUrl(): string | undefined {
    const t = environment.apiBaseUrl.trim();
    return t.length > 0 ? t : undefined;
  }

  private mockLogin(body: WebLoginRequestBody): Observable<WebLoginResult> {
    return timer(40).pipe(
      take(1),
      switchMap(() => {
        if (body.password === 'wrong') {
          return throwError(() => mockInvalidCredentialsError());
        }
        const u = body.username.trim().toLowerCase();
        if (u === 'superadmin' || u === 'groupadmin') {
          return of<WebLoginResult>({
            kind: 'mfa',
            payload: {
              mfaTicket: `mfa_${u}_${Date.now()}`,
              expiresInSeconds: 300,
            },
          });
        }
        return of<WebLoginResult>({ kind: 'session', payload: this.mockLoginOk(body.username) });
      }),
    );
  }

  private mockVerifyMfa(body: WebMfaVerifyBody): Observable<WebSessionPayload> {
    return timer(30).pipe(
      take(1),
      switchMap(() => {
        if (!body.mfaTicket.startsWith('mfa_')) {
          return throwError(() => mfaExpiredError());
        }
        if (body.totpCode !== '123456') {
          return throwError(() => mfaInvalidError());
        }
        return of<WebSessionPayload>({
          accessToken: `mock-access-mfa-${Date.now()}`,
          expiresIn: 900,
          sessionId: `sess_mfa_${Date.now()}`,
          sessionState: 'active',
        });
      }),
    );
  }

  private mockLoginOk(username: string): WebSessionPayload {
    return {
      accessToken: `mock-access-${username}-${Date.now()}`,
      expiresIn: 900,
      sessionId: `sess_mock_${Date.now()}`,
      sessionState: 'active',
    };
  }

  private mockRefreshPayload(): WebSessionPayload {
    return {
      accessToken: `mock-access-refreshed-${Date.now()}`,
      expiresIn: 900,
      sessionId: `sess_mock_${Date.now()}`,
      sessionState: 'active',
    };
  }
}

function mockInvalidCredentialsError(): AxiosError<{ code: string; message: string; requestId: string }> {
  const err = new AxiosError<{ code: string; message: string; requestId: string }>('invalid credentials');
  err.response = {
    status: 401,
    statusText: 'Unauthorized',
    data: { code: 'AUTH_INVALID_CREDENTIALS', message: 'invalid', requestId: 'mock-req' },
    headers: {},
    config: {} as never,
  };
  return err;
}

function mfaInvalidError(): AxiosError<{ code: string; message: string; requestId: string }> {
  const err = new AxiosError<{ code: string; message: string; requestId: string }>('mfa invalid');
  err.response = {
    status: 401,
    statusText: 'Unauthorized',
    data: { code: 'AUTH_MFA_INVALID', message: 'invalid', requestId: 'mock-req' },
    headers: {},
    config: {} as never,
  };
  return err;
}

function mfaExpiredError(): AxiosError<{ code: string; message: string; requestId: string }> {
  const err = new AxiosError<{ code: string; message: string; requestId: string }>('mfa expired');
  err.response = {
    status: 401,
    statusText: 'Unauthorized',
    data: { code: 'AUTH_MFA_EXPIRED', message: 'expired', requestId: 'mock-req' },
    headers: {},
    config: {} as never,
  };
  return err;
}

export function readAuthErrorCode(err: unknown): string | undefined {
  const withEnv = err as Error & { authEnvelope?: ApiEnvelope<unknown> };
  if (withEnv.authEnvelope?.code && withEnv.authEnvelope.code !== 'OK') {
    return String(withEnv.authEnvelope.code);
  }
  if (isAxiosError(err) && err.response?.data && typeof err.response.data === 'object') {
    const d = err.response.data as { code?: string };
    if (typeof d.code === 'string') {
      return d.code;
    }
  }
  return undefined;
}
