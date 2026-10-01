import { addDays, calendarRange, minuteLabel, minutesForEntryInDay, type DailyPlanTemplate, type Data, type TimeEntry } from './model';
import { Plus } from 'lucide-react';

type Props = {
  data: Data;
  date: string;
  onCreateTemplate: (start: string, end: string) => void;
  onEditTemplate: (plan: DailyPlanTemplate) => void;
  onDeleteTemplate: (plan: DailyPlanTemplate) => void;
  onCreateEntry: (date: string, start: string, end: string) => void;
  onEditEntry: (entry: TimeEntry) => void;
  onDeleteEntry: (entry: TimeEntry) => void;
};

const weekdays = ['周一', '周二', '周三', '周四', '周五', '周六', '周日'];
const pixelsPerHour = 48;
const top = (minute: number) => minute / 60 * pixelsPerHour;
const blockHeight = (start: number, end: number) => Math.max(top(end - start), 20);
const timelineHours = Array.from({ length: 25 }, (_, index) => index < 16 ? index + 8 : index - 16);
export const timelineSegments = (start: number, end: number) => {
  const daytimeStart = Math.max(start, 480), daytimeEnd = Math.min(end, 1440);
  const nightStart = Math.max(start, 0), nightEnd = Math.min(end, 480);
  return [
    ...(daytimeEnd > daytimeStart ? [{ start: daytimeStart, end: daytimeEnd, top: top(daytimeStart - 480), height: blockHeight(daytimeStart, daytimeEnd) }] : []),
    ...(nightEnd > nightStart ? [{ start: nightStart, end: nightEnd, top: top(nightStart + 960), height: blockHeight(nightStart, nightEnd) }] : []),
  ];
};

export default function WeekTimeline({ data, date, onCreateTemplate, onEditTemplate, onDeleteTemplate, onCreateEntry, onEditEntry, onDeleteEntry }: Props) {
  const [weekStart] = calendarRange(date, 'week');
  const days = Array.from({ length: 7 }, (_, index) => addDays(weekStart, index));
  const templates = [...data.dailyPlanTemplates].sort((a, b) => a.start_min - b.start_min);

  return <section className="panel week-timeline-panel" aria-label="每周时间表">
    <div className="section-head"><h2>每日规划与实际时间</h2><span className="muted">每日规划为所有周共用的模板</span></div>
    <div className="week-timeline-scroll"><div className="week-timeline">
      <div className="week-time-axis"><div className="week-time-head">时间</div><div className="week-time-labels">{timelineHours.map((hour, index) => <span key={index} className={index === 0 ? 'first' : index === 24 ? 'last' : undefined} style={{ top: top(index * 60) }}>{minuteLabel(hour * 60)}</span>)}</div></div>
      <div className="week-time-lane plan-lane"><div className="week-time-head"><span>每日规划<small>长期共用</small></span><button aria-label="添加每日规划" onClick={() => onCreateTemplate('09:00', '10:00')}><Plus size={16}/></button></div><div className="week-time-canvas">{templates.flatMap(plan => timelineSegments(plan.start_min, plan.end_min).map((segment, index) => <div key={`${plan.id}-${index}`} className="week-time-block plan-block" role="button" tabIndex={0} onClick={() => onEditTemplate(plan)} onKeyDown={event => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); onEditTemplate(plan); } }} style={{ top: segment.top, height: segment.height }} title={`${plan.title} · ${minuteLabel(plan.start_min)}—${minuteLabel(plan.end_min)}`}><strong>{plan.title}</strong><small>{minuteLabel(segment.start)}—{minuteLabel(segment.end)}</small><button title={`删除每日规划：${plan.title}`} onClick={event => { event.stopPropagation(); onDeleteTemplate(plan); }}>×</button></div>))}</div></div>
      {days.map((day, index) => {
        const entries = data.timeEntries.filter(entry => minutesForEntryInDay(entry, day) > 0);
        const dayStart = new Date(`${day}T00:00:00`).getTime();
        return <div className="week-time-lane" key={day}><div className="week-time-head"><span>{weekdays[index]}<small>{day.slice(5)} · {entries.length} 条</small></span><button aria-label={`添加${weekdays[index]}实际时间记录`} onClick={() => onCreateEntry(day, '09:00', '10:00')}><Plus size={16}/></button></div><div className="week-time-canvas">{entries.map(entry => {
          const start = Math.max(0, (new Date(entry.start_at).getTime() - dayStart) / 60000);
          const end = Math.min(1440, (new Date(entry.end_at).getTime() - dayStart) / 60000);
          const category = data.categories.find(item => item.id === entry.category_id);
          const color = category?.color || '#52D11F';
          const label = entry.title || category?.name || '实际记录';
          return timelineSegments(start, end).map((segment, segmentIndex) => <div key={`${entry.id}-${segmentIndex}`} className="week-time-block actual-block" role="button" tabIndex={0} onClick={() => onEditEntry(entry)} onKeyDown={event => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); onEditEntry(entry); } }} style={{ top: segment.top, height: segment.height, borderLeftColor: color, background: `${color}26` }} title={`${label} · ${minuteLabel(start)}—${minuteLabel(end)}`}><strong>{label}</strong><small>{minuteLabel(segment.start)}—{minuteLabel(segment.end)}</small><button title={`删除实际记录：${label}`} onClick={event => { event.stopPropagation(); onDeleteEntry(entry); }}>×</button></div>);
        })}</div></div>;
      })}
    </div></div>
  </section>;
}
