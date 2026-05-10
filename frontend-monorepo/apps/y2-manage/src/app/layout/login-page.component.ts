import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { NonNullableFormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { ActivatedRoute, Router } from '@angular/router';
import { finalize } from 'rxjs';
import { AuthApiService, readAuthErrorCode } from '../core/auth-api.service';
import { mapAuthErrorCode } from '../core/auth-errors';
import { AuthSessionService } from '../core/auth-session.service';

type LoginStep = 'password' | 'totp';

@Component({
  selector: 'app-login-page',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [ReactiveFormsModule],
  templateUrl: './login-page.component.html',
})
export class LoginPageComponent {
  private readonly fb = inject(NonNullableFormBuilder);
  private readonly authApi = inject(AuthApiService);
  private readonly session = inject(AuthSessionService);
  private readonly router = inject(Router);
  private readonly route = inject(ActivatedRoute);

  protected readonly submitting = signal(false);
  protected readonly errorText = signal<string | null>(null);
  protected readonly step = signal<LoginStep>('password');

  protected readonly form = this.fb.group({
    username: ['', [Validators.required, Validators.maxLength(128)]],
    password: ['', [Validators.required, Validators.maxLength(256)]],
    totp: ['', [Validators.pattern(/^\d{6}$/), Validators.maxLength(6)]],
  });

  protected onSubmit(): void {
    if (this.step() === 'password') {
      this.onPasswordStep();
    } else {
      this.onTotpStep();
    }
  }

  protected backToPassword(): void {
    this.session.clearPendingMfaTicket();
    this.form.controls.totp.setValue('');
    this.form.controls.totp.clearValidators();
    this.form.controls.totp.updateValueAndValidity();
    this.errorText.set(null);
    this.step.set('password');
  }

  private onPasswordStep(): void {
    if (this.form.controls.username.invalid || this.form.controls.password.invalid) {
      this.form.markAllAsTouched();
      return;
    }
    this.submitting.set(true);
    this.errorText.set(null);
    const { username, password } = this.form.getRawValue();
    this.authApi
      .login({ username, password })
      .pipe(finalize(() => this.submitting.set(false)))
      .subscribe({
        next: (result) => {
          if (result.kind === 'session') {
            this.session.applySessionPayload(result.payload);
            this.navigateAfterLogin();
            return;
          }
          this.session.setPendingMfaTicket(result.payload.mfaTicket);
          this.form.controls.totp.setValidators([
            Validators.required,
            Validators.pattern(/^\d{6}$/),
          ]);
          this.form.controls.totp.updateValueAndValidity();
          this.step.set('totp');
        },
        error: (err: unknown) => {
          const code = readAuthErrorCode(err);
          this.errorText.set(
            mapAuthErrorCode(code, err instanceof Error ? err.message : undefined),
          );
        },
      });
  }

  private onTotpStep(): void {
    if (this.form.controls.totp.invalid) {
      this.form.controls.totp.markAsTouched();
      return;
    }
    const ticket = this.session.getPendingMfaTicket();
    if (!ticket) {
      this.errorText.set('会话已失效，请返回上一步重新登录。');
      return;
    }
    this.submitting.set(true);
    this.errorText.set(null);
    const totp = this.form.controls.totp.value.trim();
    this.authApi
      .verifyMfa({ mfaTicket: ticket, totpCode: totp })
      .pipe(finalize(() => this.submitting.set(false)))
      .subscribe({
        next: (payload) => {
          this.session.applySessionPayload(payload);
          this.navigateAfterLogin();
        },
        error: (err: unknown) => {
          const code = readAuthErrorCode(err);
          this.errorText.set(
            mapAuthErrorCode(code, err instanceof Error ? err.message : undefined),
          );
        },
      });
  }

  private navigateAfterLogin(): void {
    const raw = this.route.snapshot.queryParamMap.get('returnUrl');
    const safe = raw && raw.startsWith('/') && !raw.startsWith('//') ? raw : '/dashboard';
    void this.router.navigateByUrl(safe);
  }
}
