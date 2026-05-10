/** 与仓库 `docs/auth/统一认证中心API定义-v1.md` §1 约定对齐。 */

export const AUTH_API_PREFIX = '/api/v1';

export const WEB_LOGIN_PATH = `${AUTH_API_PREFIX}/auth/web/login`;

export const WEB_REFRESH_PATH = `${AUTH_API_PREFIX}/auth/web/refresh`;

export const WEB_MFA_VERIFY_PATH = `${AUTH_API_PREFIX}/auth/web/mfa/verify`;

export type ApiResultCode = 'OK' | (string & {});

export interface ApiEnvelope<T = unknown> {
  code: ApiResultCode;
  data?: T;
  message?: string;
  requestId?: string;
}

export interface WebLoginRequestBody {
  username: string;
  password: string;
}

export interface WebSessionPayload {
  accessToken: string;
  expiresIn: number;
  sessionId: string;
  sessionState: 'active' | 'suspended_readonly' | 'revoked';
}

/** 登录第一步：需完成 TOTP 校验（见 API 文档 §3.1.1）。 */
export interface MfaRequiredPayload {
  mfaTicket: string;
  expiresInSeconds: number;
}

export interface WebMfaVerifyBody {
  mfaTicket: string;
  totpCode: string;
}

export type WebLoginResult =
  | { kind: 'session'; payload: WebSessionPayload }
  | { kind: 'mfa'; payload: MfaRequiredPayload };
