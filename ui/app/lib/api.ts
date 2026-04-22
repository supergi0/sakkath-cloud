const CONFIGURED_API_ORIGIN = process.env.NEXT_PUBLIC_API_URL?.trim().replace(/\/+$/, '') ?? '';
const LOCAL_DEV_API_ORIGIN = 'http://localhost:9000';

function isLoopbackHost(hostname: string) {
  return hostname === 'localhost' || hostname === '127.0.0.1' || hostname === '0.0.0.0';
}

export function getApiOrigin() {
  if (typeof window === 'undefined') {
    return CONFIGURED_API_ORIGIN || LOCAL_DEV_API_ORIGIN;
  }

  const currentHostIsLoopback = isLoopbackHost(window.location.hostname);

  if (CONFIGURED_API_ORIGIN) {
    try {
      const configuredUrl = new URL(CONFIGURED_API_ORIGIN);
      const configuredHostIsLoopback = isLoopbackHost(configuredUrl.hostname);

      if (!currentHostIsLoopback && configuredHostIsLoopback) {
        return '';
      }
    } catch {
      return CONFIGURED_API_ORIGIN;
    }

    return CONFIGURED_API_ORIGIN;
  }

  if (currentHostIsLoopback && window.location.port !== '9000') {
    return LOCAL_DEV_API_ORIGIN;
  }

  return '';
}

export function apiUrl(path: string) {
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  const apiOrigin = getApiOrigin();

  return apiOrigin ? `${apiOrigin}${normalizedPath}` : normalizedPath;
}