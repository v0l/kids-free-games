import { expect, test, beforeEach } from 'bun:test';

beforeEach(() => {
  globalThis.localStorage = { getItem: () => '', setItem() {}, removeItem() {} };
});

test('a successful response that is not JSON is an error, not an empty object', async () => {
  globalThis.fetch = async () => new Response('<html>oops</html>', { status: 200 });
  const { get } = await import('./api.js');
  await expect(get('/stats')).rejects.toThrow('not JSON');
});

test('an error body keeps its message and status', async () => {
  globalThis.fetch = async () => new Response(JSON.stringify({ error: 'not_found', message: 'no such player' }), { status: 404 });
  const { get } = await import('./api.js');
  const err = await get('/players/x').catch((e) => e);
  expect(err.status).toBe(404);
  expect(err.message).toBe('no such player');
});
