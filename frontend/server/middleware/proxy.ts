/**
 * Same-origin reverse proxy (Nitro server middleware).
 *
 * Two prefixes are forwarded to container-internal services so the browser
 * only ever needs the frontend origin — no published backend port, no CORS:
 *
 *   /api/**          → apiProxyTarget  (Rust backend)
 *   <cdnEndpoint>**  → cdnProxyTarget  (S3-compatible object store; the
 *                                       prefix defaults to the bucket name
 *                                       path, NUXT_PUBLIC_CDN_ENDPOINT)
 *
 * Targets are runtime config (NUXT_PUBLIC_API_PROXY_TARGET /
 * NUXT_PUBLIC_CDN_PROXY_TARGET) rather than routeRules so the built image
 * stays environment-agnostic: compose decides where the services live.
 * Both empty by default = proxy disabled, browser talks to services directly.
 *
 * Plain HTTP only in practice (no WebSocket/SSE in the app), so h3's
 * proxyRequest covers every method including multipart uploads — Nitro's
 * default body limit does not apply to streamed proxy requests.
 */
export default defineEventHandler(async (event) => {
  const path = event.path;
  const cfg = useRuntimeConfig(event);

  if (path === "/api" || path.startsWith("/api/")) {
    const target = cfg.public.apiProxyTarget;
    if (!target) return; // proxy disabled — fall through to normal handling
    return proxyRequest(event, `${target}${path}`);
  }

  const cdnTarget = cfg.public.cdnProxyTarget;
  const prefix = (cfg.public.cdnEndpoint || "/pergame/").trim();
  const prefixNoSlash = prefix.replace(/\/+$/, "");
  if (
    cdnTarget &&
    prefixNoSlash &&
    (path === prefixNoSlash || path.startsWith(`${prefixNoSlash}/`))
  ) {
    return proxyRequest(event, `${cdnTarget}${path}`);
  }
  // Anything else is the Nuxt app itself.
});
