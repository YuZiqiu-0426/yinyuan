import type { Routes } from '@angular/router';
import { authGuard } from './core/auth.guard';
import { loginShellGuard } from './core/login-shell.guard';

export const routes: Routes = [
  {
    path: 'login',
    canActivate: [loginShellGuard],
    loadComponent: () =>
      import('./layout/login-page.component').then((m) => m.LoginPageComponent),
  },
  {
    path: '',
    canActivate: [authGuard],
    loadComponent: () =>
      import('./layout/main-shell.component').then((m) => m.MainShellComponent),
    children: [
      { path: '', pathMatch: 'full', redirectTo: 'dashboard' },
      {
        path: 'dashboard',
        loadComponent: () =>
          import('./pages/dashboard-page.component').then((m) => m.DashboardPageComponent),
      },
    ],
  },
  {
    path: '**',
    loadComponent: () =>
      import('./pages/not-found.component').then((m) => m.NotFoundComponent),
  },
];
