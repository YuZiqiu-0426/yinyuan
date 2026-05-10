import { Injectable, inject } from '@angular/core';
import axios, { AxiosError } from 'axios';
import { Observable, from, map, of, switchMap, throwError, timer } from 'rxjs';
import { isAxiosError } from '@y2/shared';
import type { Y2HttpClient } from '@y2/shared';
import { environment } from '../../environments/environment';
import type { ApiEnvelope, WebLoginRequestBody, WebSessionPayload } from './auth-api.types';
import { WEB_LOGIN_PATH, WEB_REFRESH_PATH } from './auth-api.types';
import { Y2_HTTP_CLIENT } from './tokens';

function unwrapOk<T>(en: ApiEnvelope<T>): T {
  if (en.code !== 'OK' || en.data === undefined) {
    const err = new Error(en.message ?? String(en.code));
    (err as Error & { authEnvelope?: ApiEnvelope<T> }).authEnvelope = en;
    throw err;
  }
  return en.data;
}

@Injectable({ providedIn: 'root' })
export class AuthApiService {
  private readonly http = inject(Y2_HTTP_CLIENT) as Y2HttpClient;
  private readonly raw = axios.create({
    baseURL: this.baseUrl(),
    withCredentials: !environment.useAuthMock,
  });

  login(body: WebLoginRequestBody): Observable<WebSessionPayload> {
    if (environment.useAuthMock) {
      return this.mockLogin(body);
    }
    return this.http.post<ApiEnvelope<WebSessionPayload>>(WEB_LOGIN_PATH, body).pipe(
      map((en) => unwrapOk(en)),
    );
  }

  /**
   * 使用无拦截器的 axios，供 401 重试与拦截器调用，避免与 Y2HttpClient 拦截器互相递归。
   */
  refreshTokens(): Observable<WebSessionPayload> {
    if (environment.useAuthMock) {
      return timer(20).pipe(map(() => this.mockRefreshPayload()));
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

  private mockLogin(body: WebLoginRequestBody): Observable<WebSessionPayload> {
    return timer(40).pipe(
      switchMap(() => {
        if (body.password === 'wrong') {
          return throwError(() => mockInvalidCredentialsError());
        }
        return of(this.mockLoginOk(body.username));
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
