import { Injectable, computed, signal } from '@angular/core';
import { Router } from '@angular/router';
import { environment } from '../../environments/environment';
import type { WebSessionPayload } from './auth-api.types';

@Injectable({ providedIn: 'root' })
export class AuthSessionService {
  private readonly token = signal<string | null>(null);
  private readonly sessionId = signal<string | null>(null);
  private readonly accessExpiresAtMs = signal<number | null>(null);

  readonly hasToken = computed(() => {
    const t = this.token();
    return t !== null && t !== '';
  });

  constructor(private readonly router: Router) {}

  getAccessToken(): string | null {
    return this.token();
  }

  /** 受保护路由：开发绕过或已写入 access token。 */
  canAccessProtectedRoutes(): boolean {
    return environment.devBypassAuth || this.hasToken();
  }

  /** 登录页：仅当确有 token 时离开（不受 devBypass 影响）。 */
  shouldRedirectFromLogin(): boolean {
    return this.hasToken();
  }

  applySessionPayload(data: WebSessionPayload): void {
    this.token.set(data.accessToken);
    this.sessionId.set(data.sessionId);
    this.accessExpiresAtMs.set(Date.now() + data.expiresIn * 1000);
  }

  setToken(value: string): void {
    this.token.set(value);
  }

  clearToken(): void {
    this.token.set(null);
    this.sessionId.set(null);
    this.accessExpiresAtMs.set(null);
  }

  logout(): void {
    this.clearToken();
    void this.router.navigateByUrl('/login');
  }
}
