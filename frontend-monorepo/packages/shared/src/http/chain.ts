import type { AxiosInstance } from 'axios';
import { from, type Observable } from 'rxjs';
import type { Y2HttpInterceptorFn, Y2InternalRequest, Y2NextFn } from './types';

export function buildAxiosBackend(axios: AxiosInstance): Y2NextFn {
  return (req: Y2InternalRequest): Observable<import('axios').AxiosResponse<unknown>> =>
    from(
      axios.request<unknown>({
        method: req.method,
        url: req.url,
        data: req.body,
        headers: req.headers,
        params: req.params,
        responseType: req.responseType ?? 'json',
        signal: req.signal,
        timeout: req.timeout,
      })
    );
}

/**
 * 将拦截器链与末端 axios 组装为单一 `Y2NextFn`。
 * 数组顺序与 Angular 一致：**第一个**拦截器最先处理出站请求。
 */
export function composeInterceptors(
  interceptors: readonly Y2HttpInterceptorFn[],
  backend: Y2NextFn
): Y2NextFn {
  return interceptors.reduceRight<Y2NextFn>(
    (next, interceptor) => (req) => interceptor(req, next),
    backend
  );
}
