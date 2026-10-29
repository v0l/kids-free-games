import { expect, test } from 'bun:test';
import { GAMES, ago, duration, gameName, parseRoute, setGames } from './format.js';

test('durations read as seconds, minutes, then hours', () => {
  expect(duration(45)).toBe('45s');
  expect(duration(600)).toBe('10 min');
  expect(duration(3600)).toBe('1h');
  expect(duration(12226)).toBe('3h 24m');
});

test('ago buckets from just now to a date', () => {
  expect(ago(1000, 1030)).toBe('just now');
  expect(ago(1000, 1000 + 600)).toBe('10 min ago');
  expect(ago(0)).toBe('never');
});

test('routes parse from the hash, including a player on any page', () => {
  expect(parseRoute('')).toEqual({ name: 'overview' });
  expect(parseRoute('#/players')).toEqual({ name: 'players' });
  expect(parseRoute('#/players/abc')).toEqual({ name: 'player', id: 'abc' });
  expect(parseRoute('#/games/snake')).toEqual({ name: 'game', game: 'snake' });
});

test('the game list and names come from the server', () => {
  setGames([{ id: 'clash', name: 'Snake Clash' }, { id: 'trains', name: 'Take Away Trains' }]);
  expect(GAMES).toEqual(['clash', 'trains']);
  expect(gameName('trains')).toBe('Take Away Trains');
  expect(gameName('nope')).toBe('nope');
});
