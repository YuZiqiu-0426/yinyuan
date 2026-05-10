import { EnvironmentProviders, Injector, makeEnvironmentProviders } from '@angular/core';
import { environment } from '../../environments/environment';
import { AuthApiService } from './auth-api.service';
import { createAuth401Interceptor, createBearerInterceptor } from './auth-http.interceptors';
import { AuthSessionService } from './auth-session.service';
import { createY2HttpClient } from './api-client.factory';
import { Y2_HTTP_CLIENT } from './tokens';

export function provideCore(): EnvironmentProviders {
  return makeEnvironmentProviders([
    AuthSessionService,
    {
      provide: Y2_HTTP_CLIENT,
      useFactory: (auth: AuthSessionService, inj: Injector) =>
        createY2HttpClient(environment, [
          createAuth401Interceptor(auth, () => inj.get(AuthApiService)),
          createBearerInterceptor(auth),
        ]),
      deps: [AuthSessionService, Injector],
    },
  ]);
}
