/** @type {import('next').NextConfig} */
const nextConfig = {
  output: 'standalone',
  eslint: { ignoreDuringBuilds: true },
  typescript: { ignoreBuildErrors: true },
  experimental: {
    serverActions: {
      bodySizeLimit: '10mb',
    },
  },

  // Proxy all /api/* requests (except auth) to the Rust API server.
  // This avoids CORS issues and keeps auth cookies flowing through same-origin.
  // Also exposes Linear / ClickUp webhook endpoints through the same hostname,
  // so users only need to configure TUNNEL_FRONTEND_HOSTNAME for external ingest.
  // Set API_URL env at runtime: docker-compose uses http://api:8081, Vercel uses the Render URL.
  async rewrites() {
    const apiUrl = process.env.API_URL || 'http://localhost:8081';
    return [
      {
        source: '/api/:path((?!auth).*)',
        destination: `${apiUrl}/api/:path*`,
      },
      {
        source: '/webhooks/:provider(linear|clickup)',
        destination: `${apiUrl}/webhooks/:provider`,
      },
    ];
  },
};

export default nextConfig;
