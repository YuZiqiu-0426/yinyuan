import { inject } from '@angular/core';
import type { CanActivateFn } from '@angular/router';
import { Router } from '@angular/router';
import { AuthSessionService } from './auth-session.service';

export const loginShellGuard: CanActivateFn = () => {
  const auth = inject(AuthSessionService);
  const router = inject(Router);
  if (auth.shouldRedirectFromLogin()) {
    return router.createUrlTree(['/dashboard']);
  }
  return true;
};
