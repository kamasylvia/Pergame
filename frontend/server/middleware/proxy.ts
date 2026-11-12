/**
 * Same-origin reverse proxy (Nitro server middleware).
 *
 * Two prefixes are forwarded to container-internal services so the browser
 * only ever needs the frontend origin — no published backend port, no CORS:
 *   /api/**          → apiProxyTarget  (Rust backend)
 *   <cdnEndpoint>**  → cdnProxyTarget  (Rust backend: legacy /pergame/**
 *                                       embeds, authed reads; no anonymous
 *                                       S3 access required)
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
    // h3's `getProxyRequestHeaders` lists `accept` as ignored, so the
    // upstream only ever saw undici's `*/*`. That is fatal for content
    // negotiation the API does care about: `/api/mcp` answers 406 unless the
    // client accepts both application/json and text/event-stream. Forward the
    // client's accept explicitly (it wins over the merged defaults).
    const accept = event.node.req.headers.accept;
    return proxyRequest(
      event,
      `${target}${path}`,
      accept ? { headers: { accept } } : undefined,
    );
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
