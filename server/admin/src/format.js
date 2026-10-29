export const GAMES = ['blocks', 'breakout', 'count', 'crossy', 'flappy', 'fruit', 'helix', 'hotel', 'jumper', 'mathpop', 'memory', 'merge', 'paper', 'snake', 'spell', 'stack'];

const NAMES = {
  blocks: 'Blocks', breakout: 'Brick Breaker', count: 'Count It', crossy: 'Road Hopper',
  flappy: 'Flappy', fruit: 'Fruit Slice', helix: 'Helix', hotel: 'Hotel Hop',
  jumper: 'Sky Jumper', mathpop: 'Math Pop', memory: 'Memory Match', merge: '2048',
  paper: 'Paper Land', snake: 'Snake', spell: 'Spell It', stack: 'Stack Tower',
};

export const gameName = (g) => NAMES[g] || g;
export const slotName = (slot) => (slot === 'profile' ? 'Profile' : gameName(slot));

export function duration(seconds) {
  const s = Math.round(seconds);
  if (s < 60) return `${s}s`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min`;
  const h = Math.floor(s / 3600);
  const mm = Math.round((s % 3600) / 60);
  return mm ? `${h}h ${mm}m` : `${h}h`;
}

export const number = (n) => Number(n).toLocaleString();

export const date = (t) =>
  t ? new Date(t * 1000).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' }) : 'never';

export function ago(t, now = Date.now() / 1000) {
  if (!t) return 'never';
  const d = now - t;
  if (d < 90) return 'just now';
  if (d < 3600) return `${Math.round(d / 60)} min ago`;
  if (d < 86400) return `${Math.round(d / 3600)} h ago`;
  if (d < 86400 * 30) return `${Math.round(d / 86400)} days ago`;
  return date(t);
}

export function parseRoute(hash) {
  const parts = hash.replace(/^#\/?/, '').split('/').filter(Boolean);
  if (parts[0] === 'players' && parts[1]) return { name: 'player', id: parts[1] };
  if (parts[0] === 'players') return { name: 'players' };
  if (parts[0] === 'games' && parts[1]) return { name: 'game', game: parts[1] };
  return { name: 'overview' };
}
