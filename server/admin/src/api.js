const KEY = 'kg-admin-key';

export class ApiError extends Error {
  constructor(status, body) {
    super(body.message || body.error || 'request failed');
    this.status = status;
    this.code = body.error;
  }
}

export const session = {
  key: localStorage.getItem(KEY) || '',
  onSignOut: () => {},
  set(key) {
    this.key = key;
    localStorage.setItem(KEY, key);
  },
  clear() {
    this.key = '';
    localStorage.removeItem(KEY);
    this.onSignOut();
  },
};

export async function api(method, path, body) {
  const res = await fetch(`/v1/admin${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${session.key}`,
      ...(body !== undefined ? { 'Content-Type': 'application/json' } : {}),
    },
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  let data;
  try {
    data = await res.json();
  } catch {
    data = null;
  }
  if (!res.ok) {
    if (res.status === 401 && session.key) session.clear();
    throw new ApiError(res.status, data || {});
  }
  if (data === null) throw new ApiError(res.status, { message: 'the server sent something that is not JSON' });
  return data;
}

export const get = (path) => api('GET', path);
