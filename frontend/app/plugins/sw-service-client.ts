export default defineNuxtPlugin(() => {
  if (import.meta.client && 'serviceWorker' in navigator) {
    const config = useRuntimeConfig();

    // Ask for durable storage once: without it the browser may evict the
    // attachment cache under storage pressure (and Safari clears it after
    // ~7 days of non-use). A refusal is fine — caching still works.
    void (async () => {
      try {
        const storage = navigator.storage;
        if (!storage?.persist || (await storage.persisted?.())) return;
        const granted = await storage.persist();
        console.debug(`[sw] persistent storage ${granted ? 'granted' : 'not granted'}`);
      } catch {
        // Not supported / blocked: keep the best-effort cache.
      }
    })();

    navigator.serviceWorker.ready.then(registration => {
      if (registration.active) {
        registration.active.postMessage({
          type: 'SET_RUNTIME_CONFIG',
          payload: {
            BASE_URL: config.public.baseUrl,
            BASE_API: config.public.baseApi,
            BASE_CDN: config.public.baseCdn,
          },
        });
      }
    });
  }
});
