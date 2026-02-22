import { describe, it, expect } from 'vitest';
import { formatUptime } from '../utils';

describe('formatUptime', () => {
  it('returns "0s" for 0 seconds', () => {
    expect(formatUptime(0)).toBe('0s');
  });

  it('returns "1s" for 1 second', () => {
    expect(formatUptime(1)).toBe('1s');
  });

  it('returns "42s" for 42 seconds', () => {
    expect(formatUptime(42)).toBe('42s');
  });

  it('returns "59s" for 59 seconds', () => {
    expect(formatUptime(59)).toBe('59s');
  });

  it('returns "1m 0s" for 60 seconds', () => {
    expect(formatUptime(60)).toBe('1m 0s');
  });

  it('returns "1m 1s" for 61 seconds', () => {
    expect(formatUptime(61)).toBe('1m 1s');
  });

  it('returns "59m 59s" for 3599 seconds', () => {
    expect(formatUptime(3599)).toBe('59m 59s');
  });

  it('returns "1h 0m" for 3600 seconds', () => {
    expect(formatUptime(3600)).toBe('1h 0m');
  });

  it('returns "1h 1m" for 3661 seconds', () => {
    expect(formatUptime(3661)).toBe('1h 1m');
  });

  it('returns "23h 59m" for 86399 seconds', () => {
    expect(formatUptime(86399)).toBe('23h 59m');
  });

  it('returns "1d 0h" for 86400 seconds', () => {
    expect(formatUptime(86400)).toBe('1d 0h');
  });

  it('returns "1d 1h" for 90061 seconds', () => {
    expect(formatUptime(90061)).toBe('1d 1h');
  });
});
