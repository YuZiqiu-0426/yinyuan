export * from './http';

export type SessionState = "active" | "suspended_readonly" | "revoked";

export const PERMISSIONS = [
  "text.send",
  "text.recv",
  "json.send",
  "json.recv",
  "command.send",
  "command.recv",
  "file.send",
  "file.recv"
] as const;

export type PermissionCode = (typeof PERMISSIONS)[number];

export const SHARED_TEST_TAG = "shared-test-v1";

export function buildSharedTestMessage(appName: string): string {
  return `[${SHARED_TEST_TAG}] ${appName} connected to @y2/shared`;
}
