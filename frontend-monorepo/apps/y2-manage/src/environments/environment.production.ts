import type { AppEnvironment } from './environment.types';

export const environment: AppEnvironment = {
  production: true,
  apiBaseUrl: '',
  authIssuer: '',
  devBypassAuth: false,
  useAuthMock: false,
  devCsrfToken: '',
  refreshSkewSeconds: 60,
};
