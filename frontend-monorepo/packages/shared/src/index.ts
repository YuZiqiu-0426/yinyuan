export { isAxiosError } from "axios";
export { buildAxiosBackend, composeInterceptors } from './http/chain';
export { mergeAbortSignals } from './http/merge-abort-signal';
export { toY2HttpResponse } from './http/to-y2-response';
export type {
  Y2HttpInterceptorFn,
  Y2HttpOptions,
  Y2HttpParams,
  Y2HttpResponse,
  Y2InternalRequest,
  Y2NextFn,
  Y2Observe,
  Y2ResponseType,
} from './http/types';
export { Y2HttpClient } from './http/y2-http-client';

export type SessionState = "active" | "suspended_readonly" | "revoked";

export const PERMISSIONS = [
  "text.send",
  "text.recv",
  "json.send",
  "json.recv",
  "command.send",
  "command.recv",
  "file.send",
  "file.recv",
] as const;

export type PermissionCode = (typeof PERMISSIONS)[number];

export const SHARED_TEST_TAG = "shared-test-v1";

export function buildSharedTestMessage(appName: string): string {
  return `[${SHARED_TEST_TAG}] ${appName} connected to @y2/shared`;
}
