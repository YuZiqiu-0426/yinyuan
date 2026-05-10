/**
 * 合并两个 AbortSignal：任一方 abort 则结果 signal 进入 aborted。
 * 用于「Observable 退订」与「调用方传入的 signal」同时生效。
 */
export function mergeAbortSignals(
  primary: AbortSignal,
  secondary?: AbortSignal
): AbortSignal {
  if (!secondary) {
    return primary;
  }
  const merged = new AbortController();
  const forward = (): void => {
    merged.abort();
  };
  if (primary.aborted || secondary.aborted) {
    merged.abort();
    return merged.signal;
  }
  primary.addEventListener('abort', forward, { once: true });
  secondary.addEventListener('abort', forward, { once: true });
  return merged.signal;
}
