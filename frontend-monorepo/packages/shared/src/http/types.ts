import type { Observable } from 'rxjs';

/** 与 Angular `HttpClient` 对齐的子集：`body` 默认；`response` 返回状态与头。 */
export type Y2Observe = 'body' | 'response';

export type Y2HttpParams = Record<string, string | number | boolean | undefined>;

export type Y2ResponseType = 'json' | 'text' | 'blob' | 'arraybuffer';

export interface Y2HttpOptions {
  headers?: Record<string, string>;
  params?: Y2HttpParams;
  observe?: Y2Observe;
  responseType?: Y2ResponseType;
  timeout?: number;
  /** 与内部取消合并：任一 abort 都会终止请求。 */
  signal?: AbortSignal;
}

/** 链内只读请求快照；`signal` 在发起订阅时由 `Y2HttpClient` 注入。 */
export interface Y2InternalRequest {
  method: string;
  url: string;
  body?: unknown;
  headers: Record<string, string>;
  params?: Y2HttpParams;
  responseType?: Y2ResponseType;
  timeout?: number;
  signal?: AbortSignal;
}

export type Y2NextFn = (req: Y2InternalRequest) => Observable<import('axios').AxiosResponse<unknown>>;

/**
 * 与 Angular `HttpInterceptorFn` 类似：`interceptors` 数组中**靠前**的拦截器**先**收到出站请求。
 */
export type Y2HttpInterceptorFn = (
  req: Y2InternalRequest,
  next: Y2NextFn
) => Observable<import('axios').AxiosResponse<unknown>>;

/** 不直接暴露 `AxiosResponse`，便于日后换传输实现。 */
export interface Y2HttpResponse<T> {
  status: number;
  statusText: string;
  data: T;
  headers: Record<string, string>;
}
