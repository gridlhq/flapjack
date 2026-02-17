import { useSyncExternalStore } from 'react';

interface ActiveTask {
  taskID: number | string;
  indexName: string;
  documentCount: number;
  startedAt: number;
}

interface IndexingState {
  activeTasks: ActiveTask[];
}

let state: IndexingState = { activeTasks: [] };
const listeners = new Set<() => void>();

function emit() {
  listeners.forEach((l) => l());
}

export function addActiveTask(task: ActiveTask) {
  state = { activeTasks: [...state.activeTasks, task] };
  emit();
}

export function removeActiveTask(taskID: number | string) {
  state = {
    activeTasks: state.activeTasks.filter((t) => t.taskID !== taskID),
  };
  emit();
}

export function useIndexingStatus() {
  const current = useSyncExternalStore(
    (cb) => {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    () => state
  );
  return {
    activeTasks: current.activeTasks,
    isIndexing: current.activeTasks.length > 0,
    totalPending: current.activeTasks.length,
  };
}
