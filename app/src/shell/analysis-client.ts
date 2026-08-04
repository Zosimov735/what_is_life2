import type { AnalysisResult, AnalysisScenario, AnalysisTask } from './experiment';

interface Pending {
  resolve: (value: AnalysisResult) => void;
  reject: (reason: unknown) => void;
  progress: (completed: number, total: number) => void;
  fallback: () => Promise<AnalysisResult>;
}

export class AnalysisCoordinator {
  private worker: Worker | null = null;
  private next = 0;
  private pending = new Map<number, Pending>();

  constructor() {
    if (typeof Worker === 'undefined') return;
    try {
      this.worker = new Worker(new URL('./analysis-worker.ts', import.meta.url), { type: 'module' });
    } catch {
      this.worker = null;
      return;
    }
    this.worker.onmessage = (event: MessageEvent<{
      id: number;
      progress?: number;
      total?: number;
      result?: AnalysisResult;
      error?: string;
    }>) => {
      const standing = this.pending.get(event.data.id);
      if (!standing) return;
      if (event.data.error) {
        this.pending.delete(event.data.id);
        void standing.fallback().then(standing.resolve, standing.reject);
      } else if (event.data.result !== undefined) {
        this.pending.delete(event.data.id);
        standing.resolve(event.data.result);
      } else {
        standing.progress(event.data.progress ?? 0, event.data.total ?? 3);
      }
    };
  }

  run(
    task: AnalysisTask,
    scenario: AnalysisScenario,
    exportText: string,
    fallback: () => Promise<AnalysisResult>,
    progress: (completed: number, total: number) => void,
  ): Promise<AnalysisResult> {
    const id = ++this.next;
    if (!this.worker) {
      progress(0, 1);
      return fallback().then((result) => {
        progress(1, 1);
        return result;
      });
    }
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject, progress, fallback });
      this.worker?.postMessage({ id, task, scenario, exportText });
    });
  }

  cancel(): void {
    this.worker?.terminate();
    this.worker = null;
    for (const pending of this.pending.values()) pending.reject(new Error('analysis_cancelled'));
    this.pending.clear();
  }
}
