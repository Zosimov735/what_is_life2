import { execFileSync } from 'node:child_process';
import type { TestProject } from 'vitest/node';

declare module 'vitest' {
  export interface ProvidedContext {
    /** The workspace root, so a test can reach a built file on disk. */
    workspace: string;
  }
}

/**
 * Builds what the worker loads, so the tests run against the real thing rather
 * than whatever a previous build left behind: the content digest the build
 * embeds, and the module itself.
 */
export default function setup(project: TestProject): void {
  const workspace = project.config.root;
  try {
    execFileSync('npm', ['run', 'build:content'], { cwd: workspace, stdio: 'pipe' });
  } catch (cause) {
    const output = (cause as { stderr?: Buffer }).stderr?.toString() ?? '';
    throw new Error(`the content build failed\n${output}`);
  }
  try {
    execFileSync('npm', ['run', 'build:wasm'], { cwd: workspace, stdio: 'pipe' });
  } catch (cause) {
    const output = (cause as { stderr?: Buffer }).stderr?.toString() ?? '';
    throw new Error(`wasm-pack build failed\n${output}`);
  }
  project.provide('workspace', workspace);
}
