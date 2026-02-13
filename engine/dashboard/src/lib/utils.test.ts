import { describe, it, expect } from 'vitest'
import { formatBytes, formatDate, formatDuration, cn } from './utils'

describe('cn (className merger)', () => {
  it('merges class names', () => {
    const result = cn('text-red-500', 'bg-blue-500')
    expect(result).toBe('text-red-500 bg-blue-500')
  })

  it('handles conditional classes', () => {
    const result = cn('base-class', false && 'hidden', true && 'visible')
    expect(result).toBe('base-class visible')
  })

  it('resolves Tailwind conflicts (last wins)', () => {
    const result = cn('px-2 py-1', 'px-4')
    expect(result).toBe('py-1 px-4')
  })
})

describe('formatBytes', () => {
  it('formats 0 bytes', () => {
    expect(formatBytes(0)).toBe('0 Bytes')
  })

  it('formats bytes', () => {
    expect(formatBytes(1023)).toBe('1023 Bytes')
  })

  it('formats kilobytes', () => {
    expect(formatBytes(1024)).toBe('1 KB')
    expect(formatBytes(2048)).toBe('2 KB')
  })

  it('formats megabytes', () => {
    expect(formatBytes(1024 * 1024)).toBe('1 MB')
    expect(formatBytes(5 * 1024 * 1024)).toBe('5 MB')
  })

  it('formats gigabytes', () => {
    expect(formatBytes(1024 * 1024 * 1024)).toBe('1 GB')
    expect(formatBytes(2.5 * 1024 * 1024 * 1024)).toBe('2.5 GB')
  })

  it('respects decimals parameter', () => {
    expect(formatBytes(1500, 0)).toBe('1 KB')
    expect(formatBytes(1500, 1)).toBe('1.5 KB')
    expect(formatBytes(1500, 2)).toBe('1.46 KB')
  })

  it('handles negative decimals', () => {
    expect(formatBytes(1500, -1)).toBe('1 KB')
  })
})

describe('formatDate', () => {
  it('formats recent times as "just now"', () => {
    const now = new Date()
    expect(formatDate(now)).toBe('just now')

    const thirtySecsAgo = new Date(now.getTime() - 30 * 1000)
    expect(formatDate(thirtySecsAgo)).toBe('just now')
  })

  it('formats minutes ago', () => {
    const now = new Date()
    const oneMinAgo = new Date(now.getTime() - 60 * 1000)
    expect(formatDate(oneMinAgo)).toBe('1 min ago')

    const fiveMinsAgo = new Date(now.getTime() - 5 * 60 * 1000)
    expect(formatDate(fiveMinsAgo)).toBe('5 mins ago')
  })

  it('formats hours ago', () => {
    const now = new Date()
    const oneHourAgo = new Date(now.getTime() - 60 * 60 * 1000)
    expect(formatDate(oneHourAgo)).toBe('1 hour ago')

    const threeHoursAgo = new Date(now.getTime() - 3 * 60 * 60 * 1000)
    expect(formatDate(threeHoursAgo)).toBe('3 hours ago')
  })

  it('formats days ago', () => {
    const now = new Date()
    const oneDayAgo = new Date(now.getTime() - 24 * 60 * 60 * 1000)
    expect(formatDate(oneDayAgo)).toBe('1 day ago')

    const threeDaysAgo = new Date(now.getTime() - 3 * 24 * 60 * 60 * 1000)
    expect(formatDate(threeDaysAgo)).toBe('3 days ago')
  })

  it('formats older dates as locale string', () => {
    const eightDaysAgo = new Date('2026-02-05T10:00:00Z')
    const result = formatDate(eightDaysAgo)

    // Result should be a date string (varies by locale)
    expect(result).toMatch(/\d{1,2}\/\d{1,2}\/\d{4}/)
  })

  it('handles ISO string input', () => {
    const now = new Date()
    const oneMinAgo = new Date(now.getTime() - 60 * 1000)
    const isoString = oneMinAgo.toISOString()

    expect(formatDate(isoString)).toBe('1 min ago')
  })

  it('handles Date object input', () => {
    const now = new Date()
    const oneMinAgo = new Date(now.getTime() - 60 * 1000)

    expect(formatDate(oneMinAgo)).toBe('1 min ago')
  })
})

describe('formatDuration', () => {
  it('formats milliseconds', () => {
    expect(formatDuration(0)).toBe('0ms')
    expect(formatDuration(50)).toBe('50ms')
    expect(formatDuration(999)).toBe('999ms')
  })

  it('formats seconds', () => {
    expect(formatDuration(1000)).toBe('1.00s')
    expect(formatDuration(2500)).toBe('2.50s')
    expect(formatDuration(59999)).toBe('60.00s')
  })

  it('formats minutes', () => {
    expect(formatDuration(60000)).toBe('1.00m')
    expect(formatDuration(150000)).toBe('2.50m')
    expect(formatDuration(3600000)).toBe('60.00m')
  })
})
