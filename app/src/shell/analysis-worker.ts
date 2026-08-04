/// <reference lib="webworker" />

import init, { Core } from '../../../worker/wasm-pkg/field_game_core.js';
import { contentBundle } from '../../../worker/src/content';
import { PROTOCOL_VERSION, SAVE_VERSION, type Payload } from '../../../worker/src/protocol';
import type { AnalysisResult, AnalysisScenario, AnalysisTask } from './experiment';

const scope = self as DedicatedWorkerGlobalScope;
const ready = init();

type CoreResponse =
  | { ok: true; body: Payload }
  | { ok: false; error: { code: string; message_key: string; detail: unknown } };

function call(core: Core, command: string, body: Payload): CoreResponse {
  return JSON.parse(core.command(command, JSON.stringify(body))) as CoreResponse;
}

scope.onmessage = (
  event: MessageEvent<{
    id: number;
    task: AnalysisTask;
    scenario: AnalysisScenario;
    exportText: string;
  }>,
) => {
  void (async () => {
    const { id, task, scenario, exportText } = event.data;
    try {
      scope.postMessage({ id, progress: 0, total: 3 });
      await ready;
      const core = new Core(JSON.stringify({
        content: contentBundle(),
        protocol: PROTOCOL_VERSION,
        save_version: SAVE_VERSION,
      }));
      scope.postMessage({ id, progress: 1, total: 3 });
      const imported = call(core, 'import_run', { text: exportText });
      if (!imported.ok) throw new Error(imported.error.code);
      scope.postMessage({ id, progress: 2, total: 3 });
      const analyzed = call(core, 'run_analysis', { kind: task, scenario });
      if (!analyzed.ok) throw new Error(analyzed.error.code);
      scope.postMessage({ id, progress: 3, total: 3 });
      scope.postMessage({ id, result: analyzed.body as unknown as AnalysisResult });
    } catch (cause) {
      scope.postMessage({
        id,
        error: cause instanceof Error ? cause.message : 'analysis_failed',
      });
    }
  })();
};

export {};
