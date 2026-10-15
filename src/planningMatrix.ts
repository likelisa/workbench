import type { Data, Priority, Status } from './model';
import { cycleTodos, itemHierarchy, sortItems, taskItems, type Item } from './unifiedModel';

export type PlanningGroup = { id: string; title: string; column: number };
export type PlanningRow = {
  id: string;
  path: PlanningGroup[];
  starts: (PlanningGroup & { span: number })[];
  todo?: Item;
  folded?: boolean;
  unassigned?: boolean;
  annualId?: string | null;
  goalId?: string | null;
};

export function planningRows(data: Data, cycleId: string, collapsed: Set<string>): PlanningRow[] {
  const todos = cycleTodos(data).filter(i => i.cycle_id === cycleId || data.placements.some(p => p.item_id === i.id && p.cycle_id === cycleId));
  const goals = data.cycleGoals.filter(g => g.cycle_id === cycleId);
  const annualIds = [...new Set([...todos.map(i => i.annual_goal_id), ...goals.map(g => g.annual_goal_id)])];
  const directionOf = (annualId: string | null) => data.annualGoals.find(g => g.id === annualId)?.direction_id || null;
  const rows: PlanningRow[] = [];
  const push = (row: Omit<PlanningRow, 'starts'>) => rows.push({ ...row, starts: [] });
  for (const directionId of new Set(annualIds.map(directionOf))) {
    const direction = { id: `dir:${directionId}`, title: data.directions.find(d => d.id === directionId)?.title || '未归属年度方向', column: 0 };
    if (collapsed.has(direction.id)) { push({ id: direction.id, path: [direction], folded: true }); continue; }
    for (const annualId of annualIds.filter(a => directionOf(a) === directionId)) {
      const annual = { id: `annual:${annualId}`, title: data.annualGoals.find(g => g.id === annualId)?.title || '未归属年度目标', column: 1 };
      if (collapsed.has(annual.id)) { push({ id: annual.id, path: [direction, annual], folded: true }); continue; }
      const annualGoals = goals.filter(g => g.annual_goal_id === annualId);
      const buckets = [...annualGoals.map(g => ({ id: g.id, title: g.title })),
        ...(todos.some(i => i.annual_goal_id === annualId && !annualGoals.some(g => g.id === i.cycle_goal_id)) ? [{ id: null, title: '未归属本轮目标' }] : [])];
      for (const bucket of buckets) {
        const goal = { id: `goal:${annualId}:${bucket.id}`, title: bucket.title, column: 2 };
        const path = [direction, annual, goal];
        if (collapsed.has(goal.id)) { push({ id: goal.id, path, folded: true, annualId, goalId: bucket.id }); continue; }
        const children = todos.filter(i => i.annual_goal_id === annualId && (bucket.id === null ? !annualGoals.some(g => g.id === i.cycle_goal_id) : i.cycle_goal_id === bucket.id));
        if (!children.length) push({ id: `empty:${goal.id}`, path, annualId, goalId: bucket.id });
        for (const todo of children) push({ id: todo.id, path, todo, annualId, goalId: bucket.id });
      }
    }
  }
  rows.forEach((row, index) => {
    row.path.forEach((group, column) => {
      if (rows[index - 1]?.path[column]?.id === group.id) return;
      let span = 1;
      while (rows[index + span]?.path[column]?.id === group.id) span++;
      row.starts.push({ ...group, span });
    });
  });
  push({ id: 'unassigned', path: [], unassigned: true });
  return rows;
}

export function planningGeometry(scale: number, viewportWidth: number) {
  const week = 164 * scale;
  const tree = Math.min(620 * scale, Math.max(0, viewportWidth - week - 2));
  const columns = [110, 160, 160, 190].map(width => width / 620 * tree);
  return { tree, week, columns, lefts: columns.map((_, index) => columns.slice(0, index).reduce((a, b) => a + b, 0)) };
}

export type PoolKind = 'day' | 'week' | 'cycle';
export const POOL_LABELS: Record<PoolKind, string> = { day: '日任务池', week: '周任务池', cycle: '本轮待办池' };
export function unscheduledPool(data: Data, kind: PoolKind, cycleId?: string): Item[] {
  return sortItems(taskItems(data).filter(item => {
    if (item.level !== kind) return false;
    const placements = data.placements.filter(p => p.item_id === item.id);
    if (cycleId && item.cycle_id && item.cycle_id !== cycleId && !placements.some(p => p.cycle_id === cycleId)) return false;
    return kind === 'cycle' ? placements.length === 0 : !placements.some(p => p.kind === kind);
  }));
}

export type TaskPoolFilters = { arrangement: 'all' | 'scheduled' | 'unscheduled'; direction: string; annualGoal: string; status: '' | Status; priority: '' | Priority };
export const DEFAULT_POOL_FILTERS: TaskPoolFilters = { arrangement: 'all', direction: '', annualGoal: '', status: '', priority: '' };
export const UNLINKED_POOL_FILTER = '__unlinked__';
export const ALL_POOL_LABELS: Record<PoolKind, string> = { ...POOL_LABELS, cycle: '本轮待办任务池' };
export function poolAnnualGoals(data: Data, direction: string) {
  return data.annualGoals.filter(goal => !direction || (direction === UNLINKED_POOL_FILTER ? !data.directions.some(d => d.id === goal.direction_id) : goal.direction_id === direction));
}
export function normalizePoolFilters(data: Data, filters: TaskPoolFilters): TaskPoolFilters {
  const direction = !filters.direction || filters.direction === UNLINKED_POOL_FILTER || data.directions.some(d => d.id === filters.direction) ? filters.direction : '';
  const annualGoal = !filters.annualGoal || filters.annualGoal === UNLINKED_POOL_FILTER || poolAnnualGoals(data, direction).some(g => g.id === filters.annualGoal) ? filters.annualGoal : '';
  return { ...filters, direction, annualGoal };
}
export function taskPool(data: Data, kind: PoolKind, cycleId: string, filters: TaskPoolFilters): Item[] {
  const matches = (filter: string, value: string | null) => !filter || (filter === UNLINKED_POOL_FILTER ? !value : filter === value);
  return sortItems(taskItems(data).filter(item => {
    if (item.level !== kind || (filters.status && item.status !== filters.status) || (filters.priority && (item.priority || 'P2') !== filters.priority)) return false;
    const hierarchy = itemHierarchy(data, item.id);
    const placements = data.placements.filter(p => p.item_id === item.id);
    const cycles = new Set([item.cycle_id, hierarchy.cycle_id, ...placements.map(p => p.cycle_id)].filter(Boolean));
    if (cycles.size && !cycles.has(cycleId)) return false;
    const scheduled = placements.some(p => kind === 'cycle' || p.kind === kind);
    if (filters.arrangement !== 'all' && scheduled !== (filters.arrangement === 'scheduled')) return false;
    const goal = data.annualGoals.find(g => g.id === hierarchy.annual_goal_id);
    const direction = data.directions.find(d => d.id === goal?.direction_id);
    return matches(filters.annualGoal, goal?.id || null) && matches(filters.direction, direction?.id || null);
  }));
}
