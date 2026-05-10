import type { AxiosInstance } from 'axios';
import { Observable } from 'rxjs';
import { buildAxiosBackend, composeInterceptors } from './chain';
import { mergeAbortSignals } from './merge-abort-signal';
import { toY2HttpResponse } from './to-y2-response';
import type {
  Y2HttpInterceptorFn,
  Y2HttpOptions,
  Y2HttpResponse,
  Y2InternalRequest,
  Y2NextFn,
  Y2Observe,
} from './types';

/**
 * 基于 axios 执行、对外返回 RxJS `Observable`，方法形态参考 Angular `HttpClient` 子集。
 * 浏览器 bundle；未针对 Node SSR 做适配。
 */
export class Y2HttpClient {
  private readonly dispatch: Y2NextFn;

  constructor(
    private readonly axios: AxiosInstance,
    interceptors: readonly Y2HttpInterceptorFn[] = []
  ) {
    const backend = buildAxiosBackend(axios);
    this.dispatch = composeInterceptors(interceptors, backend);
  }

  request<T>(method: string, url: string, body?: unknown, options?: Y2HttpOptions): Observable<T>;
  request<T>(
    method: string,
    url: string,
    body: unknown | undefined,
    options: Y2HttpOptions & { observe: 'response' }
  ): Observable<Y2HttpResponse<T>>;
  request<T>(
    method: string,
    url: string,
    body?: unknown,
    options?: Y2HttpOptions
  ): Observable<T | Y2HttpResponse<T>> {
    const observe: Y2Observe = options?.observe ?? 'body';
    return this.runPipeline(method, url, body, options, observe);
  }

  get<T>(url: string, options?: Y2HttpOptions): Observable<T>;
  get<T>(url: string, options: Y2HttpOptions & { observe: 'response' }): Observable<Y2HttpResponse<T>>;
  get<T>(url: string, options?: Y2HttpOptions): Observable<T | Y2HttpResponse<T>> {
    return this.request('GET', url, undefined, options);
  }

  delete<T>(url: string, options?: Y2HttpOptions): Observable<T>;
  delete<T>(url: string, options: Y2HttpOptions & { observe: 'response' }): Observable<Y2HttpResponse<T>>;
  delete<T>(url: string, options?: Y2HttpOptions): Observable<T | Y2HttpResponse<T>> {
    return this.request('DELETE', url, undefined, options);
  }

  post<T>(url: string, body: unknown, options?: Y2HttpOptions): Observable<T>;
  post<T>(
    url: string,
    body: unknown,
    options: Y2HttpOptions & { observe: 'response' }
  ): Observable<Y2HttpResponse<T>>;
  post<T>(url: string, body: unknown, options?: Y2HttpOptions): Observable<T | Y2HttpResponse<T>> {
    return this.request('POST', url, body, options);
  }

  put<T>(url: string, body: unknown, options?: Y2HttpOptions): Observable<T>;
  put<T>(
    url: string,
    body: unknown,
    options: Y2HttpOptions & { observe: 'response' }
  ): Observable<Y2HttpResponse<T>>;
  put<T>(url: string, body: unknown, options?: Y2HttpOptions): Observable<T | Y2HttpResponse<T>> {
    return this.request('PUT', url, body, options);
  }

  patch<T>(url: string, body: unknown, options?: Y2HttpOptions): Observable<T>;
  patch<T>(
    url: string,
    body: unknown,
    options: Y2HttpOptions & { observe: 'response' }
  ): Observable<Y2HttpResponse<T>>;
  patch<T>(url: string, body: unknown, options?: Y2HttpOptions): Observable<T | Y2HttpResponse<T>> {
    return this.request('PATCH', url, body, options);
  }

  private runPipeline<T>(
    method: string,
    url: string,
    body: unknown | undefined,
    options: Y2HttpOptions | undefined,
    observe: Y2Observe
  ): Observable<T | Y2HttpResponse<T>> {
    return new Observable((subscriber) => {
      const subscriptionAbort = new AbortController();
      const signal = mergeAbortSignals(subscriptionAbort.signal, options?.signal);
      const req: Y2InternalRequest = {
        method,
        url,
        body,
        headers: { ...options?.headers },
        params: options?.params,
        responseType: options?.responseType,
        timeout: options?.timeout,
        signal,
      };
      const sub = this.dispatch(req).subscribe({
        next: (res) => {
          if (observe === 'response') {
            subscriber.next(toY2HttpResponse(res) as Y2HttpResponse<T>);
          } else {
            subscriber.next(res.data as T);
          }
          subscriber.complete();
        },
        error: (err: unknown) => {
          subscriber.error(err);
        },
      });
      return () => {
        subscriptionAbort.abort();
        sub.unsubscribe();
      };
    });
  }
}
