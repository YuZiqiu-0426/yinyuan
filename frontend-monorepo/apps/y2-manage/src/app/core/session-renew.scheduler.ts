import { effect, inject, Injectable, untracked } from '@angular/core';
import { catchError, of, take } from 'rxjs';
import { environment } from '../../environments/environment';
import { AuthApiService } from './auth-api.service';
import { AuthSessionService } from './auth-session.service';

/**
 * 在 access 过期前 `refreshSkewSeconds` 触发一次静默 refresh（与 401 拦截器共用 `refreshTokens` 单飞）。
 */
@Injectable({ providedIn: 'root' })
export class SessionRenewScheduler {
  private readonly auth = inject(AuthSessionService);
  private readonly api = inject(AuthApiService);
  private timerId: ReturnType<typeof setTimeout> | null = null;

  constructor() {
    effect(() => {
      const exp = this.auth.expiresAtMs();
      untracked(() => this.arm(exp));
    });
  }

  private arm(exp: number | null): void {
    if (this.timerId !== null) {
      clearTimeout(this.timerId);
      this.timerId = null;
    }
    if (exp === null) {
      return;
    }
    const skewMs = environment.refreshSkewSeconds * 1000;
    const delay = Math.max(0, exp - skewMs - Date.now());
    this.timerId = setTimeout(() => this.fire(), delay);
  }

  private fire(): void {
    this.timerId = null;
    this.api
      .refreshTokens()
      .pipe(
        take(1),
        catchError(() => of(null)),
      )
      .subscribe((payload) => {
        if (payload) {
          this.auth.applySessionPayload(payload);
        } else {
          this.auth.logout();
        }
      });
  }
}
