/** 将 `docs/auth/统一认证中心API定义-v1.md` §6 错误码映射为中文。 */

const MESSAGES: Record<string, string> = {
  AUTH_INVALID_CREDENTIALS: '用户名或密码不正确。',
  AUTH_TOKEN_EXPIRED: '登录已过期，请重新登录。',
  AUTH_REFRESH_EXPIRED: '刷新会话失败，请重新登录。',
  AUTH_IP_MISMATCH: '登录环境异常（IP 不一致）。',
  AUTH_DEVICE_UNTRUSTED: '设备未受信任。',
  AUTH_CLI_REVIEW_PENDING: 'CLI 设备待审核。',
  AUTH_CLI_REVIEW_REJECTED: 'CLI 审核未通过。',
  AUTH_PERMISSION_DENIED: '没有权限执行该操作。',
  AUTH_SESSION_READONLY: '会话为只读，无法发送或管理。',
  AUTH_SESSION_REVOKED: '会话已失效，请重新登录。',
  AUTH_GROUP_SCOPE_DENIED: '组范围校验失败。',
  AUTH_CSRF_INVALID: '安全校验失败（CSRF），请刷新页面后重试。',
  AUTH_RISK_REVOKED: '检测到高风险行为，会话已终止。',
};

export function mapAuthErrorCode(code: string | undefined, fallbackMessage?: string): string {
  if (code && MESSAGES[code]) {
    return MESSAGES[code];
  }
  if (fallbackMessage) {
    return fallbackMessage;
  }
  return '请求失败，请稍后重试。';
}
