import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { NonNullableFormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { ActivatedRoute, Router } from '@angular/router';
import { finalize } from 'rxjs';
import { AuthApiService, readAuthErrorCode } from '../core/auth-api.service';
import { mapAuthErrorCode } from '../core/auth-errors';
import { AuthSessionService } from '../core/auth-session.service';

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

  protected readonly form = this.fb.group({
    username: ['', [Validators.required, Validators.maxLength(128)]],
    password: ['', [Validators.required, Validators.maxLength(256)]],
  });

  protected onSubmit(): void {
    if (this.form.invalid) {
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
        next: (payload) => {
          this.session.applySessionPayload(payload);
          const raw = this.route.snapshot.queryParamMap.get('returnUrl');
          const safe =
            raw && raw.startsWith('/') && !raw.startsWith('//') ? raw : '/dashboard';
          void this.router.navigateByUrl(safe);
        },
        error: (err: unknown) => {
          const code = readAuthErrorCode(err);
          this.errorText.set(
            mapAuthErrorCode(code, err instanceof Error ? err.message : undefined),
          );
        },
      });
  }
}
