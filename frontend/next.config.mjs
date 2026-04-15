/** @type {import('next').NextConfig} */
const nextConfig = {
  output: 'standalone',
  eslint: { ignoreDuringBuilds: true },
  typescript: { ignoreBuildErrors: true },

  // Proxy all /api/* requests (except auth) to the Rust API server.
  // This avoids CORS issues and keeps auth cookies flowing through same-origin.
  // Set API_URL env at runtime: docker-compose uses http://api:8081, Vercel uses the Render URL.
  async rewrites() {
    const apiUrl = process.env.API_URL || 'http://localhost:8081';
    return [
      {
        source: '/api/:path((?!auth).*)',
        destination: `${apiUrl}/api/:path*`,
      },
    ];
  },
};

export default nextConfig;
