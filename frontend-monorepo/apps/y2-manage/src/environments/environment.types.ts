export interface AppEnvironment {
  production: boolean;
  apiBaseUrl: string;
  authIssuer: string;
  /** 仅本地开发：跳过受保护路由的 token 检查；勿用于生产部署。 */
  devBypassAuth: boolean;
  /**
   * 为 true 时登录/刷新走内存 Mock，不调真实网络；与文档 JSON 形状一致。
   * 生产必须为 false。
   */
  useAuthMock: boolean;
  /**
   * Mock 或非 HttpOnly 场景占位；真实环境由服务端下发 CSRF 后写入（Phase 2+ 再接）。
   */
  devCsrfToken: string;
  /** 临期主动 refresh：在 access 过期前多少秒触发（与 SessionRenewScheduler 一致）。 */
  refreshSkewSeconds: number;
}
