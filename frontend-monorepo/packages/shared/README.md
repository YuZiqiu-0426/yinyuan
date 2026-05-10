# `@y2/shared`

跨 `frontend-monorepo` 应用的共享库（权限常量、HTTP 封装等）。

## HTTP：`Y2HttpClient`（axios + RxJS）

- 使用 **axios** 发请求，对外返回 **RxJS `Observable`**，方法名与选项参考 Angular `HttpClient` 的**子集**（`get` / `post` / `put` / `patch` / `delete` / `request`，`observe: 'body' | 'response'`）。
- **拦截器**：`Y2HttpInterceptorFn`，数组中**靠前**的**先**处理出站请求（与 Angular 一致）。可用 `composeInterceptors` / `buildAxiosBackend` 自定义链。
- **取消订阅**：内部使用 `AbortController`；若传入 `options.signal`，与退订合并，任一 abort 即终止请求。
- **运行环境**：面向浏览器 bundle；未针对 Node SSR 适配。

示例：

```ts
import axios from 'axios';
import { Y2HttpClient } from '@y2/shared';

const api = new Y2HttpClient(axios.create({ baseURL: '/api' }), [
  (req, next) => next({ ...req, headers: { ...req.headers, 'X-Trace': '1' } }),
]);

api.get<{ ok: boolean }>('/health').subscribe(console.log);
```

错误处理可配合 `isAxiosError`（从 `@y2/shared` 再导出）。
