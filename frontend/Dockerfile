# ========================================
# Build stage
# ========================================
FROM oven/bun:1 AS builder

WORKDIR /app

# Dépendances (cacheable).
# BUN_REGISTRY lets a deployment point at a reachable mirror; the default
# stays the public registry so CI behaves as before.
ARG BUN_REGISTRY=https://registry.npmjs.org
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile --registry "$BUN_REGISTRY"

# Code source
COPY . .

ENV NODE_ENV=production
RUN bun run build
# ========================================
# Runtime stage
# ========================================
FROM node:26-alpine AS runtime

WORKDIR /app

COPY --from=builder /app/.output /app/.output
COPY docker-entrypoint.sh /app/docker-entrypoint.sh
RUN chmod +x /app/docker-entrypoint.sh

EXPOSE 8200
CMD ["/app/docker-entrypoint.sh"]
