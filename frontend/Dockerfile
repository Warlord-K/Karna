FROM node:22-alpine AS builder
WORKDIR /app

# Install deps (lock file ensures reproducible builds)
COPY package.json package-lock.json* ./
RUN npm ci --ignore-scripts 2>/dev/null || npm install

# Copy source
COPY . .

# Ensure public dir exists (COPY fails if source dir is missing)
RUN mkdir -p public

# Provide dummy DATABASE_URL so Next.js build can compile server modules
# that import pg Pool at module level (auth.ts, lib/db.ts).
# The real value is injected at runtime via docker-compose environment.
ENV DATABASE_URL=postgres://build:build@localhost:5432/build

# Build
RUN npm run build

FROM node:22-alpine AS runner
WORKDIR /app
ENV NODE_ENV=production

# Copy standalone build output
COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static
COPY --from=builder /app/public ./public

EXPOSE 3000
CMD ["node", "server.js"]
