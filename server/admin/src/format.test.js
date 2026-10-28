import { expect, test } from 'bun:test';
import { ago, duration, parseRoute } from './format.js';

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
