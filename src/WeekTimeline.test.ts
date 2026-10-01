import { describe, expect, it } from 'vitest';
import { timelineSegments } from './WeekTimeline';

describe('周时间轴从 08:00 开始', () => {
  it('把凌晨时段放在同一天的下方', () => {
    expect(timelineSegments(0, 420)).toEqual([{ start: 0, end: 420, top: 768, height: 336 }]);
    expect(timelineSegments(540, 600)).toEqual([{ start: 540, end: 600, top: 48, height: 48 }]);
  });

  it('跨过 08:00 的记录在上下两处显示', () => {
    expect(timelineSegments(420, 540)).toEqual([
      { start: 480, end: 540, top: 0, height: 48 },
      { start: 420, end: 480, top: 1104, height: 48 },
    ]);
  });
});
