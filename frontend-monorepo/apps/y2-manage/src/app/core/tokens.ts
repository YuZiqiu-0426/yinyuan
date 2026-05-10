import { InjectionToken } from '@angular/core';
import type { Y2HttpClient } from '@y2/shared';

export const Y2_HTTP_CLIENT = new InjectionToken<Y2HttpClient>('Y2_HTTP_CLIENT');
