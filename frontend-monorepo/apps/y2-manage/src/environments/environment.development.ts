import type { AppEnvironment } from './environment.types';

export const environment: AppEnvironment = {
  production: false,
  apiBaseUrl: 'http://127.0.0.1:8081',
  authIssuer: 'http://127.0.0.1:8081',
  devBypassAuth: false,
  useAuthMock: true,
  devCsrfToken: 'dev-mock-csrf',
  refreshSkewSeconds: 10,
};
