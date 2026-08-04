/**
 * The shell's surfaces: the neutral notice while the worker opens, the Field
 * once it has answered, and the notice a resumed run surfaces after a worker
 * fault.
 *
 * The shell is chrome. It owns the notices, the surface element, and the
 * session behind it; the renderer owns everything drawn on that element and
 * reads nothing but the snapshots the worker sends.
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { Atlas, type RegimeId } from './Atlas';
import {
  advanceEngineeringOperation,
  auditEngineeringOperations,
  commissionAttempts,
  engineeringBlueprints,
  engineeringGeneratorSources,
  engineeringMigrationJournal,
  qualificationCriterionDecisions,
  qualificationFunctionDecision,
  qualificationFailureTrace,
  qualificationGrades,
  qualificationJob,
  qualificationResultGroup,
  qualificationUnlockReceipts,
  qualificationTrials,
  prepareEngineeringAssemblyOperation,
  prepareEngineeringTransitionOperation,
  publishEngineeringActiveSession,
  storeCommissionAttempt,
  storeEngineeringAssemblyCommit,
  storeEngineeringTransitionCommit,
  storeEngineeringMemoryCapture,
  storeQualificationJob,
  storeQualificationCriterionDecision,
  storeQualificationFunctionDecision,
  storeQualificationFailureTrace,
  storeQualificationGrade,
  storeQualificationResultGroup,
  storeQualificationUnlockReceipt,
  storeQualificationRequest,
  storeQualificationTrial,
  storeRunLineage,
  storeAutomationSessionSave,
  type CommissionAttemptRecord,
  type CommissionClosure,
  type CommissionGeneratorDiff,
  type CommissionRestartBoundary,
  type CommissionWeakestMargin,
  type EngineeringBlueprintEntry,
  type EngineeringBlueprintThumbnail,
  type EngineeringGeneratorSourceEntry,
  type EngineeringMigrationJournal,
  type EngineeringOperationRecovery,
} from './archive';
import { AutomationWorkbench } from './AutomationWorkbench';
import type { EngineeringTransitionCompanion } from '../render';
import { ContractLadder } from './ContractLadder';
import { copy } from './copy';
import { ExperimentLab } from './ExperimentLab';
import { FieldSurface, type FieldInspection } from './FieldSurface';
import { FormSelect } from './FormSelect';
import { openSound, type Sound } from './sound';
import type { StillTool } from './still-edits';
import {
  openCore,
  type CandidateSlate,
  type CommissionBreakpoint,
  type CoreClient,
  type FramePair,
  type MechanismTimelineEntry,
  type QueueState,
  type RunIdentity,
} from './worker-client';
import type {
  ContractCatalog,
  ContractCatalogEntry,
  DesignCommitted,
  EngineeringMemoryCapture,
  EngineeringAssemblyCommitResult,
  EngineeringAssemblyDraft,
  EngineeringAssemblyPreview,
  EngineeringRunTransitionPreview,
  EngineeringTransitionCommitResult,
  EngineeringTransitionKind,
  FormId,
  FrozenLocalPolicy,
  PlanCommitted,
  PolicyPreview,
  QualificationFrozen,
  QualificationCriterionDecision,
  QualificationFunctionDecision,
  QualificationFailureTrace,
  QualificationFailureTraceResult,
  QualificationGrade,
  QualificationGrades,
  QualificationInputPreview,
  QualificationJob,
  QualificationResolution,
  QualificationResultGroup,
  QualificationReceiptResult,
  QualificationUnlockReceipt,
  QualificationTrialArtifact,
  RouteControlDefault,
  RunExported,
  ViewDeclaration,
} from '../../../worker/src/protocol';
import { PROTOCOL_VERSION } from '../../../worker/src/protocol';
import type { FrameState } from '../../../worker/src/frame-state';
import type { CriterionReading, PressureState } from '../../../worker/src/protocol';

interface ShellProps {
  /**
   * How the worker session is started, on the Form the opening surface took.
   * Replaced in tests.
   */
  open?: (form: FormId, regime?: RegimeId) => CoreClient;
  /**
   * How the cues are sounded. Replaced in tests, and `null` opens none at all.
   *
   * The level it opens at is the locked `InputConfig` default, full, because
   * `input_config` does not cross the frame boundary yet — the same standing
   * carry-forward the trail intensity the renderer takes stands on. The goal
   * that adds the settings surface hands both their configured values.
   */
  sound?: (() => Sound) | null;
}

/**
 * Where a local preview reaches the session, so a whole cycle — open, run,
 * pause, export, restart, import, resume — can be driven from the page without
 * a control surface the player will never see. Developer diagnostics, and only
 * in a development build.
 */
const BRIDGE_HANDLE = 'field_game_bridge';

/**
 * The query marker a local preview draws the development stand-in Field with,
 * in place of the empty Field a run stands on until authored content arrives.
 * Development builds only: the branch that reads it, and the module it reaches,
 * are both dropped from a production build.
 */
const FIXTURE_MARKER = 'field_fixture';

/**
 * The catalog key a refusal names, and none for a developer-only fault. The
 * envelope carries it; the shell never chooses one.
 */
function noticeKey(cause: unknown): string | null {
  if (typeof cause !== 'object' || cause === null) return null;
  const held = cause as { ok?: boolean; error?: { message_key?: string | null } };
  if (held.ok !== false) return null;
  return held.error?.message_key ?? null;
}

/**
 * The candidate the authoritative passive View matches, 1-based, and 0 while
 * it matches none. Focusing never reads or mutates the causal plan queue.
 */
function focusedIn(view: ViewDeclaration | null, slate: CandidateSlate | null): number {
  if (!view || !slate) return 0;
  return (
    slate.candidates.find(
      (candidate) =>
        candidate.view.resolution === view.resolution &&
        candidate.view.window === view.window &&
        candidate.view.surround === view.surround &&
        candidate.view.inside.length === view.inside.length &&
        candidate.view.inside.every((node, place) => node === view.inside[place]),
    )?.position ?? 0
  );
}

/**
 * How a diagnostic session is opened when a caller supplies the legacy hook.
 *
 * It stands here rather than as a default written into the parameter list
 * because the effect that opens a session names this function among the things
 * it watches. A function rebuilt on every render is a new thing to watch every
 * render, and the effect would end the run and open another one each time —
 * a whole worker and a whole `init_run` per redraw. Normal product startup
 * bypasses this adapter and opens a catalog-only worker below.
 */
function openSession(form: FormId, regime: RegimeId = 'open_field'): CoreClient {
  return openCore({ form, regime });
}

interface PreparedCommissionClosure {
  exported: RunExported;
  identity: RunIdentity;
  record: CommissionAttemptRecord;
}

function captureBlueprintThumbnail(
  surface: HTMLCanvasElement | null,
  identity: RunIdentity | null,
): EngineeringBlueprintThumbnail | null {
  if (
    !surface
    || !identity?.assemblyHash
    || !identity.generatorHash
    || identity.embodiedHash !== identity.assemblyHash
  ) return null;
  try {
    const width = 384;
    const height = 216;
    const thumbnail = document.createElement('canvas');
    thumbnail.width = width;
    thumbnail.height = height;
    const context = thumbnail.getContext('2d');
    if (!context || surface.width === 0 || surface.height === 0) return null;
    context.fillStyle = '#071013';
    context.fillRect(0, 0, width, height);
    const scale = Math.min(width / surface.width, height / surface.height);
    const drawnWidth = surface.width * scale;
    const drawnHeight = surface.height * scale;
    context.drawImage(
      surface,
      (width - drawnWidth) / 2,
      (height - drawnHeight) / 2,
      drawnWidth,
      drawnHeight,
    );
    return {
      assemblyHash: identity.assemblyHash,
      dataUrl: thumbnail.toDataURL('image/webp', 0.82),
      generatorHash: identity.generatorHash,
      height,
      projectionVersion: 1,
      width,
    };
  } catch {
    return null;
  }
}

function firstConsequenceOrdinal(events: readonly MechanismTimelineEntry[]): number | null {
  return events[0]?.ordinal ?? null;
}

function mechanismAddress(entry: MechanismTimelineEntry): { target: string; id: number } | null {
  const event = entry.event;
  const target = event.kind === 'policy'
    ? event.object_kind
    : event.kind === 'interface'
      ? 'node'
      : event.kind === 'route'
        ? 'route'
        : event.kind === 'supply'
          ? 'current'
          : event.kind === 'reserve'
            ? 'form'
            : event.kind === 'charge' && event.dominant_node !== null
              ? 'node'
              : null;
  const id = event.kind === 'policy'
    ? event.object_id
    : event.kind === 'interface'
      ? event.node
      : event.kind === 'route'
        ? event.route
        : event.kind === 'supply'
          ? event.current
          : event.kind === 'reserve'
            ? event.form
            : event.kind === 'charge'
              ? event.dominant_node
              : null;
  return target === null || id === null ? null : { target, id };
}

function weakestCriterionMargin(reading: CriterionReading | null): CommissionWeakestMargin | null {
  if (!reading) return null;
  const candidates: Array<CommissionWeakestMargin & { normalized: number }> = [];
  for (const component of reading.components) {
    candidates.push({
      kind: 'component',
      objectId: component.node,
      margin: component.margin,
      measured: component.charge,
      required: component.minimum_q,
      normalized: component.margin / Math.max(1, Math.abs(component.minimum_q)),
    });
  }
  for (const route of reading.routes) {
    const margin = route.minimum - route.floor;
    candidates.push({
      kind: 'route',
      objectId: route.route,
      margin,
      measured: route.minimum,
      required: route.floor,
      normalized: margin / Math.max(1, Math.abs(route.floor)),
    });
  }
  if (reading.leakage.ratio !== null) {
    const margin = reading.leakage.ceiling - reading.leakage.ratio;
    candidates.push({
      kind: 'leakage',
      objectId: null,
      margin,
      measured: reading.leakage.ratio,
      required: reading.leakage.ceiling,
      normalized: margin / Math.max(1, Math.abs(reading.leakage.ceiling)),
    });
  }
  const handsOffRequired = reading.hands_off_streak + reading.hands_off_remaining;
  if (handsOffRequired > 0) {
    candidates.push({
      kind: 'hands_off',
      objectId: null,
      margin: reading.hands_off_streak - handsOffRequired,
      measured: reading.hands_off_streak,
      required: handsOffRequired,
      normalized: -reading.hands_off_remaining / handsOffRequired,
    });
  }
  candidates.sort((left, right) => left.normalized - right.normalized);
  const weakest = candidates[0];
  if (!weakest) return null;
  const { normalized: _normalized, ...margin } = weakest;
  return margin;
}

function closureFailure(reason: string) {
  return {
    v: PROTOCOL_VERSION,
    re: 0,
    ok: false as const,
    error: {
      code: 'internal' as const,
      message_key: 'automation.attempts.persistence_error',
      detail: { reason },
    },
  };
}

export function App({ open = openSession, sound = openSound }: ShellProps) {
  const [regime, setRegime] = useState<RegimeId | null>(() => {
    if (open !== openSession) return 'open_field';
    if (import.meta.env.DEV && typeof window !== 'undefined') {
      const query = new URLSearchParams(window.location.search);
      if (query.has('field_run') || query.has(FIXTURE_MARKER)) return 'open_field';
    }
    return null;
  });
  /** Legacy diagnostic Form selection. Normal product entry is catalog-first. */
  const [chosen, setChosen] = useState<FormId | null>(null);
  const [client, setClient] = useState<CoreClient | null>(null);
  const [ready, setReady] = useState(false);
  const [recovering, setRecovering] = useState(false);
  const [fixture, setFixture] = useState<(() => FramePair) | null>(null);
  const [sounding, setSounding] = useState<Sound | null>(null);
  const [criterion, setCriterion] = useState<CriterionReading | null>(null);
  const [pressures, setPressures] = useState<PressureState[]>([]);
  const [refused, setRefused] = useState<string | null>(null);
  const [mode, setMode] = useState<FrameState['header']['mode'] | null>(null);
  const [queue, setQueue] = useState<QueueState | null>(null);
  const [slate, setSlate] = useState<CandidateSlate | null>(null);
  const [view, setView] = useState<ViewDeclaration | null>(null);
  const [tool, setTool] = useState<StillTool>('view');
  const [policy, setPolicy] = useState<FrozenLocalPolicy>({ version: 2, components: [] });
  const [routeDefaults, setRouteDefaults] = useState<RouteControlDefault[]>([]);
  const [mechanismEvents, setMechanismEvents] = useState<MechanismTimelineEntry[]>([]);
  const [identity, setIdentity] = useState<RunIdentity | null>(null);
  const [contractCatalog, setContractCatalog] = useState<ContractCatalog | null>(null);
  const [contractId, setContractId] = useState<string | null>(null);
  const [contractsOpen, setContractsOpen] = useState(true);
  const [commissionHistory, setCommissionHistory] = useState<CommissionAttemptRecord[]>([]);
  const [blueprints, setBlueprints] = useState<EngineeringBlueprintEntry[]>([]);
  const [generatorSources, setGeneratorSources] = useState<EngineeringGeneratorSourceEntry[]>([]);
  const [generatorSourceReadError, setGeneratorSourceReadError] = useState(false);
  const [engineeringMigration, setEngineeringMigration] = useState<EngineeringMigrationJournal | null>(null);
  const [engineeringRecoveries, setEngineeringRecoveries] = useState<EngineeringOperationRecovery[]>([]);
  const [commissionArchiveError, setCommissionArchiveError] = useState(false);
  const [qualificationRequestArchiveError, setQualificationRequestArchiveError] = useState(false);
  const [qualificationExecutionArchiveError, setQualificationExecutionArchiveError] = useState(false);
  const [qualificationJobState, setQualificationJobState] = useState<QualificationJob | null>(null);
  const [qualificationTrialArtifacts, setQualificationTrialArtifacts] = useState<QualificationTrialArtifact[]>([]);
  const [qualificationCriterionDecisionState, setQualificationCriterionDecisionState] = useState<QualificationCriterionDecision[]>([]);
  const [qualificationFunctionDecisionState, setQualificationFunctionDecisionState] = useState<QualificationFunctionDecision | null>(null);
  const [qualificationGradeState, setQualificationGradeState] = useState<QualificationGrade[]>([]);
  const [qualificationFailureTraceState, setQualificationFailureTraceState] = useState<QualificationFailureTrace | null>(null);
  const [qualificationResultState, setQualificationResultState] = useState<QualificationResultGroup | null>(null);
  const [qualificationReceiptState, setQualificationReceiptState] = useState<QualificationUnlockReceipt | null>(null);
  const [commissionBreakpoint, setCommissionBreakpoint] = useState<CommissionBreakpoint | null>(null);
  const [commissionBreakpointHit, setCommissionBreakpointHit] = useState<MechanismTimelineEntry | null>(null);
  const [policyPreview, setPolicyPreview] = useState<PolicyPreview | null>(null);
  const [assemblyPreview, setAssemblyPreview] = useState<EngineeringAssemblyPreview | null>(null);
  const [engineeringTransition, setEngineeringTransition] = useState<EngineeringTransitionCompanion | null>(null);
  const [selection, setSelection] = useState<FieldInspection | null>(null);
  const [inspectionStep, setInspectionStep] = useState<number | null>(null);
  const [selectedEventOrdinal, setSelectedEventOrdinal] = useState<number | null>(null);
  const [rate, setRate] = useState<1 | 4 | 16>(1);
  const [labOpen, setLabOpen] = useState(false);
  const branchOpeningSteps = useRef(new Map<string, number>());
  const fieldCanvas = useRef<HTMLCanvasElement | null>(null);
  const closedBranches = useRef(new Set<string>());
  const storedQualificationArtifacts = useRef(new Set<string>());
  const qualificationPersistenceChain = useRef<Promise<void>>(Promise.resolve());
  const pendingQualificationResult = useRef<QualificationResultGroup | null>(null);
  const pendingQualificationReceipt = useRef<QualificationUnlockReceipt | null>(null);
  const holdFieldSurface = useCallback((surface: HTMLCanvasElement | null): void => {
    fieldCanvas.current = surface;
  }, []);
  const selectedId = selection && 'id' in selection ? selection.id : null;
  const selectedTarget = selection?.target ?? null;
  const activeContract = contractCatalog?.contracts.find((contract) => contract.id === contractId)
    ?? null;

  useEffect(() => {
    const normalContractEntry = open === openSession;
    if (!normalContractEntry && !chosen) return;
    const session = normalContractEntry
      ? openCore({})
      : open(chosen as FormId, regime ?? undefined);
    setClient(session);
    let watching = true;
    const syncSession = (): void => {
      if (!watching) return;
      setRecovering(session.recovering());
      setCriterion(session.criterion?.() ?? null);
      setPressures(session.pressures());
      // The same for the Still Mode surface: the mode is the worker's, the
      // queue is the core's, and the Impulse rides the frame header. Nothing
      // here is derived from a key the shell saw a player press.
      setMode(session.mode());
      setQueue(session.queue());
      setPolicy(session.policy());
      setRouteDefaults(session.routeDefaults());
      setContractId(session.contractId());
      setMechanismEvents(session.mechanismEvents());
      setCommissionBreakpoint(session.commissionBreakpoint());
      setCommissionBreakpointHit(session.commissionBreakpointHit());
      setIdentity(session.identity?.() ?? null);
      const activeQualificationJob = session.qualificationJob();
      if (activeQualificationJob) {
        setQualificationJobState(activeQualificationJob);
      }
      const deliveredArtifacts = session.qualificationArtifacts();
      if (deliveredArtifacts.length > 0) {
        setQualificationTrialArtifacts((held) => {
          const merged = new Map(held.map((artifact) => [artifact.artifact_id, artifact]));
          for (const artifact of deliveredArtifacts) merged.set(artifact.artifact_id, artifact);
          return [...merged.values()].sort((left, right) => left.trial - right.trial);
        });
      }
      const pendingArtifacts = deliveredArtifacts.filter((artifact) => (
        !storedQualificationArtifacts.current.has(artifact.artifact_id)
      ));
      for (const artifact of pendingArtifacts) {
        storedQualificationArtifacts.current.add(artifact.artifact_id);
      }
      if (activeQualificationJob || pendingArtifacts.length > 0) {
        qualificationPersistenceChain.current = qualificationPersistenceChain.current
          .then(async () => {
            for (const artifact of pendingArtifacts) await storeQualificationTrial(artifact);
            if (activeQualificationJob) {
              if (
                activeQualificationJob.status === 'completed'
                && storedQualificationArtifacts.current.size
                  < activeQualificationJob.completed_trials.length
              ) throw new Error('qualification_trial_persistence_incomplete');
              await storeQualificationJob(activeQualificationJob);
            }
          })
          .catch(() => {
            for (const artifact of pendingArtifacts) {
              storedQualificationArtifacts.current.delete(artifact.artifact_id);
            }
            setQualificationExecutionArchiveError(true);
          });
      }
      setRate(session.rate());
      setSlate(session.slate());
      setView(session.view?.() ?? null);
    };
    const unwatch = session.watch(syncSession);
    session.ready.then(
      () => {
        if (!watching) return;
        // A warm worker can complete initialization before React has installed
        // its first observer. Read the standing state once at the readiness
        // boundary so fast reloads cannot lose the opening chapter/objective.
        syncSession();
        void qualificationUnlockReceipts()
          .then((receipts) => session.contracts(receipts))
          .catch(() => session.contracts([]))
          .then((catalog) => {
            if (watching && 'contract_version' in catalog) setContractCatalog(catalog);
          });
        void engineeringBlueprints().then((records) => {
          if (watching) setBlueprints(records);
        });
        void engineeringMigrationJournal().then((journal) => {
          if (watching) setEngineeringMigration(journal);
        });
        void auditEngineeringOperations().then(
          (outcomes) => {
            if (!watching) return;
            setEngineeringRecoveries(outcomes);
            if (outcomes.some((outcome) => outcome.status === 'manual_recovery')) {
              setCommissionArchiveError(true);
            }
            if (outcomes.some((outcome) => outcome.status === 'recovered')) {
              void engineeringBlueprints().then((records) => {
                if (watching) setBlueprints(records);
              });
            }
          },
          () => {
            if (watching) setCommissionArchiveError(true);
          },
        );
        setReady(true);
      },
      (cause: unknown) => {
        console.error('field_game shell: the worker session did not open', cause);
        // A session that never opened has one thing to say, and the error
        // envelope names which: `message_key` is a catalog key for a fault a
        // player is shown and null for a developer-only one. Content that does
        // not validate is the fault this goal makes reachable.
        const key = noticeKey(cause);
        if (watching && key) setRefused(key);
      },
    );
    if (import.meta.env.DEV) {
      (globalThis as Record<string, unknown>)[BRIDGE_HANDLE] = session;
    }
    return () => {
      watching = false;
      unwatch();
      if (import.meta.env.DEV) {
        delete (globalThis as Record<string, unknown>)[BRIDGE_HANDLE];
      }
      session.close();
    };
  }, [open, chosen, regime]);

  useEffect(() => {
    const attempt = identity?.attemptRecord;
    const branch = identity?.attemptBranch;
    if (
      !attempt
      || !branch
      || !identity?.attemptId
      || !identity.branchId
      || attempt.attempt_id !== identity.attemptId
      || branch.branch_id !== identity.branchId
    ) {
      setGeneratorSources([]);
      setGeneratorSourceReadError(false);
      return;
    }
    let current = true;
    void engineeringGeneratorSources({
      attemptId: identity.attemptId,
      branchId: identity.branchId,
      contentHash: attempt.content_hash,
      contractId: attempt.contract_id,
      currentBranch: branch,
    }).then(
      (sources) => {
        if (!current) return;
        setGeneratorSources(sources);
        setGeneratorSourceReadError(false);
      },
      () => {
        if (!current) return;
        setGeneratorSources([]);
        setGeneratorSourceReadError(true);
      },
    );
    return () => {
      current = false;
    };
  }, [
    identity?.attemptId,
    identity?.branchId,
    identity?.attemptBranch?.parent_branch_id,
    identity?.generatorHash,
    blueprints,
    identity?.qualificationRequestId,
    qualificationResultState?.result.result_id,
  ]);

  // The sound stands for the life of the shell rather than of one surface: it
  // holds the audio context, and a context rebuilt with every surface would
  // ask the autoplay policy for a fresh gesture each time.
  useEffect(() => {
    if (!sound) return;
    const opened = sound();
    setSounding(opened);
    return () => {
      setSounding(null);
      opened.close();
    };
  }, [sound]);

  // A completed ranking is heard. It is the one cue the sound module cannot
  // read off a snapshot: a ranking is a record the worker raises rather than a
  // place on the Field, so the ordinal of the record the run stands under is
  // what is handed over, and the module sounds a fresh one once.
  useEffect(() => {
    sounding?.ranked(slate?.ordinal ?? null);
  }, [sounding, slate]);

  useEffect(() => {
    if (!import.meta.env.DEV || typeof window === 'undefined') return;
    if (!new URLSearchParams(window.location.search).has(FIXTURE_MARKER)) return;
    let wanted = true;
    void import('./dev-frames').then((stand) => {
      if (wanted) setFixture(() => stand.fixtureFrames);
    });
    return () => {
      wanted = false;
    };
  }, []);

  // `C` and `V` select the same two tools as the focusable buttons. Interactive
  // chrome keeps its own keys; these bindings apply only to the Field surface.
  useEffect(() => {
    if (mode !== 'still' || typeof window === 'undefined') return;
    const choose = (event: KeyboardEvent): void => {
      const target = event.target as HTMLElement | null;
      if (target?.closest('button, input, select, textarea, [contenteditable="true"]')) return;
      if (event.code === 'KeyC') setTool('compartment');
      else if (event.code === 'KeyV') setTool('view');
      else return;
      if (event.cancelable) event.preventDefault();
    };
    window.addEventListener('keydown', choose);
    return () => window.removeEventListener('keydown', choose);
  }, [mode]);

  useEffect(() => {
    if (!ready || !client || selection || mode !== 'still') return;
    const form = client.snapshot()?.forms[0];
    if (!form) return;
    let current = true;
    void client.command('inspect_field', { id: form.id, target: 'form' }).then((answer) => {
      if (current && answer.ok) setSelection(answer.body as unknown as FieldInspection);
    });
    return () => {
      current = false;
    };
  }, [client, mode, ready, selection]);

  useEffect(() => {
    if (!client || !selection || !('id' in selection) || inspectionStep !== null) return;
    const target = selection.target;
    const id = selection.id;
    let current = true;
    let reading = false;
    const refresh = (): void => {
      if (reading) return;
      reading = true;
      void client.command('inspect_field', { id, target }).then((answer) => {
        reading = false;
        if (current && answer.ok) setSelection(answer.body as unknown as FieldInspection);
      });
    };
    const timer = window.setInterval(refresh, 250);
    return () => {
      current = false;
      window.clearInterval(timer);
    };
  }, [client, inspectionStep, selectedId, selectedTarget]);

  useEffect(() => {
    if (!identity?.branchId || !client) return;
    if (!branchOpeningSteps.current.has(identity.branchId)) {
      branchOpeningSteps.current.set(identity.branchId, client.snapshot()?.header.step ?? 0);
    }
    void storeRunLineage(identity).catch(() => setCommissionArchiveError(true));
  }, [client, identity?.branchId]);

  useEffect(() => {
    const request = identity?.qualificationRequest;
    if (!request) return;
    void storeQualificationRequest(request).then(
      () => setQualificationRequestArchiveError(false),
      () => setQualificationRequestArchiveError(true),
    );
  }, [identity?.qualificationRequestId]);

  useEffect(() => {
    const requestId = identity?.qualificationRequestId;
    let current = true;
    setQualificationJobState(null);
    setQualificationTrialArtifacts([]);
    setQualificationCriterionDecisionState([]);
    setQualificationFunctionDecisionState(null);
    setQualificationGradeState([]);
    setQualificationFailureTraceState(null);
    setQualificationResultState(null);
    setQualificationReceiptState(null);
    pendingQualificationResult.current = null;
    pendingQualificationReceipt.current = null;
    storedQualificationArtifacts.current.clear();
    if (!requestId) return () => { current = false; };
    void qualificationJob(requestId).then(async (stored) => {
      if (!current || !stored) return;
      const trials = await qualificationTrials(stored.job.job_id);
      if (!current) return;
      const recoveredJob: QualificationJob = ['running', 'cancel_requested'].includes(stored.job.status)
        ? { ...stored.job, status: 'interrupted' }
        : stored.job;
      setQualificationJobState(recoveredJob);
      if (recoveredJob !== stored.job) {
        void storeQualificationJob(recoveredJob).catch(() => setQualificationExecutionArchiveError(true));
      }
      setQualificationTrialArtifacts(trials.map((row) => row.artifact));
      storedQualificationArtifacts.current = new Set(trials.map((row) => row.id));
      const criterionDecisions = await qualificationCriterionDecisions(recoveredJob.job_id);
      if (!current) return;
      const functionDecision = await qualificationFunctionDecision(recoveredJob.job_id);
      if (!current) return;
      const retainedDecisions = criterionDecisions.map((row) => row.decision);
      const retainedIds = new Set(retainedDecisions.map((decision) => decision.decision_id));
      const completeFunctionDecision = functionDecision
        && functionDecision.decision.definition.criterion_decision_ids
          .every((decisionId) => retainedIds.has(decisionId))
        ? functionDecision.decision
        : null;
      setQualificationCriterionDecisionState(retainedDecisions);
      setQualificationFunctionDecisionState(completeFunctionDecision);
      if (functionDecision && !completeFunctionDecision) {
        setQualificationExecutionArchiveError(true);
      }
      const grades = await qualificationGrades(recoveredJob.job_id);
      if (!current) return;
      const retainedGrades = grades.map((row) => row.grade);
      const gradeAxes = new Set(retainedGrades.map((grade) => grade.definition.axis));
      const completeGrades = completeFunctionDecision
        && retainedGrades.length === 4
        && gradeAxes.size === 4
        && retainedGrades.every((grade) => (
          grade.definition.function_decision_id
            === completeFunctionDecision.function_decision_id
        ));
      setQualificationGradeState(completeGrades ? retainedGrades : []);
      if (retainedGrades.length > 0 && !completeGrades) {
        setQualificationExecutionArchiveError(true);
      }
      const failureTrace = await qualificationFailureTrace(recoveredJob.job_id);
      if (!current) return;
      const completeFailureTrace = completeFunctionDecision
        && failureTrace
        && failureTrace.trace.definition.function_decision_id
          === completeFunctionDecision.function_decision_id
        ? failureTrace.trace
        : null;
      setQualificationFailureTraceState(completeFailureTrace);
      if (failureTrace && !completeFailureTrace) {
        setQualificationExecutionArchiveError(true);
      }
      const storedResult = await qualificationResultGroup(recoveredJob.job_id);
      if (!current) return;
      const candidateGroup: QualificationResultGroup | null = storedResult ? {
        complete_marker: storedResult.completeMarker.marker,
        result: storedResult.result.result,
        status: 'complete',
        version: 1,
      } : null;
      const definition = candidateGroup?.result.definition;
      const artifactIds = new Set(trials.map((row) => row.id));
      const decisionIds = new Set(retainedDecisions.map((decision) => decision.decision_id));
      const gradeIds = new Set(retainedGrades.map((grade) => grade.grade_id));
      const expectedTraceId = completeFunctionDecision?.definition.passed
        ? null
        : completeFailureTrace?.failure_trace_id ?? null;
      const expectedChildCount = trials.length
        + retainedDecisions.length
        + retainedGrades.length
        + 3
        + Number(expectedTraceId !== null);
      const completeResult = candidateGroup
        && definition
        && completeFunctionDecision
        && completeGrades
        && definition.request_id === requestId
        && definition.job_id === recoveredJob.job_id
        && definition.function_decision_id === completeFunctionDecision.function_decision_id
        && definition.failure_trace_id === expectedTraceId
        && definition.artifact_ids.length === artifactIds.size
        && definition.artifact_ids.every((id) => artifactIds.has(id))
        && definition.criterion_decision_ids.length === decisionIds.size
        && definition.criterion_decision_ids.every((id) => decisionIds.has(id))
        && definition.grade_ids.length === gradeIds.size
        && definition.grade_ids.every((id) => gradeIds.has(id))
        && candidateGroup.complete_marker.definition.result_id === candidateGroup.result.result_id
        && candidateGroup.complete_marker.definition.child_count === expectedChildCount
        ? candidateGroup
        : null;
      setQualificationResultState(completeResult);
      if (candidateGroup && !completeResult) setQualificationExecutionArchiveError(true);
      const receipts = await qualificationUnlockReceipts();
      if (!current) return;
      setQualificationReceiptState(
        completeResult
          ? receipts.find((receipt) => (
            receipt.definition.result_id === completeResult.result.result_id
          )) ?? null
          : null,
      );
      const catalog = await client?.contracts(receipts);
      if (current && catalog && 'contract_version' in catalog) setContractCatalog(catalog);
    }).catch(() => {
      if (current) setQualificationExecutionArchiveError(true);
    });
    return () => { current = false; };
  }, [identity?.qualificationRequestId]);

  useEffect(() => {
    let current = true;
    if (!contractId) {
      setCommissionHistory([]);
      return () => { current = false; };
    }
    void commissionAttempts(contractId).then(
      (records) => {
        if (current) setCommissionHistory(records);
      },
      () => {
        if (current) setCommissionArchiveError(true);
      },
    );
    return () => { current = false; };
  }, [contractId]);

  useEffect(() => {
    if (!client || !commissionBreakpointHit) return;
    const addressed = mechanismAddress(commissionBreakpointHit);
    setInspectionStep(commissionBreakpointHit.step);
    setSelectedEventOrdinal(commissionBreakpointHit.ordinal);
    if (!addressed) return;
    let current = true;
    void client.command('inspect_field', {
      id: addressed.id,
      step: commissionBreakpointHit.step,
      target: addressed.target,
    }).then((answer) => {
      if (current && answer.ok) setSelection(answer.body as unknown as FieldInspection);
    });
    return () => { current = false; };
  }, [client, commissionBreakpointHit?.ordinal]);

  const prepareCommissionClosure = async (
    closure: CommissionClosure,
    restartBoundary: CommissionRestartBoundary,
  ): Promise<PreparedCommissionClosure | null> => {
    if (!client) return null;
    const closingIdentity = client.identity?.() ?? identity;
    const closingContractId = client.contractId();
    if (
      !closingIdentity
      || closingIdentity.runKind !== 'automation_contract'
      || !closingIdentity.attemptRecord
      || !closingIdentity.attemptBranch
      || !closingIdentity.attemptId
      || !closingIdentity.branchId
      || !closingIdentity.assemblyHash
      || !closingContractId
    ) return null;
    if (closedBranches.current.has(closingIdentity.branchId)) return null;

    const exported = await client.command('export_run', {});
    if (!exported.ok) throw new Error('commission_closure_export_failed');
    const exportBody = exported.body as RunExported;
    if (!exportBody.embodied_state_hash) throw new Error('commission_closure_hash_missing');
    const closingEvents = client.mechanismEvents();
    const closingCriterion = client.criterion?.() ?? criterion;
    const closingStep = client.snapshot()?.header.step ?? closingCriterion?.step ?? 0;
    const openingStep = branchOpeningSteps.current.get(closingIdentity.branchId) ?? closingStep;
    return {
      exported: exportBody,
      identity: closingIdentity,
      record: {
        id: closingIdentity.branchId,
        schemaVersion: 1,
        contractId: closingContractId,
        attemptId: closingIdentity.attemptId,
        branchId: closingIdentity.branchId,
        parentBranchId: closingIdentity.parentBranchId,
        branchNonce: closingIdentity.branchNonce,
        branchOperation: closingIdentity.attemptBranch.operation,
        runKind: closingIdentity.runKind,
        contentHash: closingIdentity.attemptRecord.content_hash,
        generatorHash: closingIdentity.generatorHash,
        assemblyHash: closingIdentity.assemblyHash,
        scenarioHash: closingIdentity.scenarioHash,
        regimeId: activeContract?.opening.regime ?? 'open_field',
        protocolVersion: PROTOCOL_VERSION,
        closingEmbodiedHash: exportBody.embodied_state_hash,
        openingStep,
        closingStep,
        recordedAt: Date.now(),
        closure,
        restartBoundary,
        generatorDiff: null,
        firstConsequenceOrdinal: firstConsequenceOrdinal(closingEvents),
        weakestMargin: weakestCriterionMargin(closingCriterion),
        events: closingEvents,
        criterion: closingCriterion,
      },
    };
  };

  const persistCommissionClosure = async (
    prepared: PreparedCommissionClosure | null,
    generatorDiff: CommissionGeneratorDiff | null = null,
  ): Promise<void> => {
    if (!prepared) return;
    const record = { ...prepared.record, generatorDiff };
    closedBranches.current.add(record.branchId);
    setCommissionHistory((held) => (
      held.some((entry) => entry.id === record.id)
        ? held
        : [record, ...held].sort((left, right) => right.recordedAt - left.recordedAt)
    ));
    try {
      await storeRunLineage(prepared.identity);
      await storeCommissionAttempt(record);
    } catch {
      setCommissionArchiveError(true);
    }
  };

  const prepareOrFault = async (
    closure: CommissionClosure,
    boundary: CommissionRestartBoundary,
  ): Promise<PreparedCommissionClosure | null | false> => {
    try {
      return await prepareCommissionClosure(closure, boundary);
    } catch {
      setCommissionArchiveError(true);
      return false;
    }
  };

  // Legacy diagnostics retain Atlas/Form entry. Normal product entry already
  // has an idle worker, but no RunState exists until the ladder opens a contract.
  if (open !== openSession) {
    if (!regime) {
      return <Atlas onOpen={setRegime} />;
    }
    if (!chosen) {
      return (
        <FormSelect
          regime={regime}
          onChoose={setChosen}
          onBack={() => setRegime(null)}
        />
      );
    }
  }
  if (refused) {
    return <p className="notice">{copy(refused)}</p>;
  }
  if (!ready || !client) {
    return <p className="notice">{copy('notice.preparing')}</p>;
  }
  if (!contractCatalog) {
    return <p className="notice">{copy('notice.preparing')}</p>;
  }
  if (contractsOpen || !contractId) {
    return (
      <ContractLadder
        catalog={contractCatalog}
        activeId={contractId}
        onReturn={contractId ? async () => {
          const answer = await client.resumeCommission();
          if (!answer.ok) return false;
          setContractsOpen(false);
          setSelection(null);
          setInspectionStep(null);
          setSelectedEventOrdinal(null);
          setPolicyPreview(null);
          return true;
        } : undefined}
        onOpen={async (contract: ContractCatalogEntry) => {
          const prepared = await prepareOrFault('superseded', null);
          if (prepared === false) return false;
          const answer = await client.openContract(contract.id);
          if (!answer.ok) return false;
          await persistCommissionClosure(prepared);
          setContractId(contract.id);
          setContractsOpen(false);
          setSelection(null);
          setInspectionStep(null);
          setSelectedEventOrdinal(null);
          setPolicyPreview(null);
          return true;
        }}
      />
    );
  }
  const snapshot = client.snapshot();
  const activeRegime = activeContract?.opening.regime ?? regime ?? 'open_field';
  return (
    <>
      <FieldSurface
        regime={activeRegime}
        frames={fixture ?? client.frames}
        sound={sounding}
        queuePlan={client.queuePlan}
        undoPlan={client.undoPlan}
        tool={tool}
        setFocus={(slateOrdinal, position) => {
          void client.setFocus?.(slateOrdinal, position);
        }}
        slate={slate}
        focused={focusedIn(view, slate)}
        inspectField={async (target, id) => {
          const answer = await client.command('inspect_field', { id, target });
          return answer.ok ? answer.body as unknown as FieldInspection : null;
        }}
        onSelect={(nextSelection) => {
          setInspectionStep(null);
          setSelectedEventOrdinal(null);
          setSelection(nextSelection);
        }}
        onSurface={holdFieldSurface}
        policyPreview={policyPreview}
        assemblyPreview={assemblyPreview}
        engineeringTransition={engineeringTransition}
      />
      <AutomationWorkbench
        contract={activeContract}
        identity={identity}
        mode={mode}
        rate={rate}
        step={snapshot?.header.step ?? 0}
        queue={queue ?? client.queue()}
        tool={tool}
        policy={policy}
        routeDefaults={routeDefaults}
        selection={selection}
        routes={snapshot?.routes ?? []}
        criterion={criterion}
        pressures={pressures}
        mechanismEvents={mechanismEvents}
        commissionHistory={commissionHistory}
        commissionArchiveError={commissionArchiveError}
        qualificationRequestArchiveError={qualificationRequestArchiveError}
        qualificationExecutionArchiveError={qualificationExecutionArchiveError}
        qualificationJob={qualificationJobState}
        qualificationTrialArtifacts={qualificationTrialArtifacts}
        qualificationCriterionDecisions={qualificationCriterionDecisionState}
        qualificationFunctionDecision={qualificationFunctionDecisionState}
        qualificationGrades={qualificationGradeState}
        qualificationFailureTrace={qualificationFailureTraceState}
        qualificationResult={qualificationResultState}
        qualificationReceipt={qualificationReceiptState}
        blueprints={blueprints}
        generatorSources={generatorSources}
        generatorSourceReadError={generatorSourceReadError}
        engineeringMigration={engineeringMigration}
        engineeringRecoveries={engineeringRecoveries}
        commissionBreakpoint={commissionBreakpoint}
        commissionBreakpointHit={commissionBreakpointHit}
        inspectionStep={inspectionStep}
        selectedEventOrdinal={selectedEventOrdinal}
        onDesign={() => {
          setInspectionStep(null);
          setSelectedEventOrdinal(null);
          void client.setDesignMode(true);
        }}
        onCommission={() => {
          setInspectionStep(null);
          setSelectedEventOrdinal(null);
          void client.setDesignMode(false);
        }}
        onPreviewRestart={client.previewCommissionRestart}
        onPreviewQualification={client.previewQualificationInput}
        onFreezeQualification={async (preview: QualificationInputPreview) => {
          const prepared = await prepareOrFault('qualified', 'current_embodied');
          if (!prepared) return closureFailure('qualification_closure_prepare_failed');
          const answer = await client.freezeQualificationRequest(preview);
          if (!answer.ok) return answer;
          await persistCommissionClosure(prepared);
          const frozen = answer.body as QualificationFrozen;
          try {
            await storeQualificationRequest(frozen.qualification_request);
            setQualificationRequestArchiveError(false);
          } catch {
            setQualificationRequestArchiveError(true);
          }
          setInspectionStep(null);
          setSelectedEventOrdinal(null);
          setPolicyPreview(null);
          return answer;
        }}
        onRetryQualificationPersistence={async () => {
          const request = client.identity?.()?.qualificationRequest;
          if (!request) return;
          try {
            await storeQualificationRequest(request);
            setQualificationRequestArchiveError(false);
          } catch {
            setQualificationRequestArchiveError(true);
          }
        }}
        onRetryQualificationExecutionPersistence={async () => {
          const requestId = client.identity?.()?.qualificationRequestId;
          if (!requestId) return;
          try {
            let job = qualificationJobState;
            let artifacts = qualificationTrialArtifacts;
            if (!job) {
              const stored = await qualificationJob(requestId);
              if (stored) {
                job = stored.job;
                const retained = await qualificationTrials(job.job_id);
                artifacts = retained.map((row) => row.artifact);
                setQualificationJobState(job);
                setQualificationTrialArtifacts(artifacts);
                storedQualificationArtifacts.current = new Set(retained.map((row) => row.id));
              }
            }
            for (const artifact of artifacts) {
              if (!job || artifact.job_id === job.job_id) await storeQualificationTrial(artifact);
            }
            if (job) await storeQualificationJob(job);
            for (const decision of qualificationCriterionDecisionState) {
              if (!job || decision.definition.job_id === job.job_id) {
                await storeQualificationCriterionDecision(decision);
              }
            }
            if (
              qualificationFunctionDecisionState
              && (!job || qualificationFunctionDecisionState.definition.job_id === job.job_id)
            ) {
              await storeQualificationFunctionDecision(qualificationFunctionDecisionState);
            }
            for (const grade of qualificationGradeState) {
              if (!job || grade.definition.job_id === job.job_id) {
                await storeQualificationGrade(grade);
              }
            }
            if (
              qualificationFailureTraceState
              && (!job || qualificationFailureTraceState.definition.job_id === job.job_id)
            ) {
              await storeQualificationFailureTrace(qualificationFailureTraceState);
            }
            const resultGroup = qualificationResultState ?? pendingQualificationResult.current;
            if (
              resultGroup
              && (!job || resultGroup.result.definition.job_id === job.job_id)
            ) {
              await storeQualificationResultGroup(resultGroup);
              setQualificationResultState(resultGroup);
              pendingQualificationResult.current = null;
            }
            const receipt = qualificationReceiptState ?? pendingQualificationReceipt.current;
            if (receipt) {
              await storeQualificationUnlockReceipt(receipt);
              setQualificationReceiptState(receipt);
              pendingQualificationReceipt.current = null;
              const receipts = await qualificationUnlockReceipts();
              const catalog = await client.contracts(receipts);
              if ('contract_version' in catalog) setContractCatalog(catalog);
            }
            setQualificationExecutionArchiveError(false);
          } catch {
            setQualificationExecutionArchiveError(true);
          }
        }}
        onStartQualification={async () => {
          const requestId = client.identity?.()?.qualificationRequestId;
          if (!requestId) return closureFailure('qualification_request_missing');
          const completedTrials = qualificationTrialArtifacts
            .filter((artifact) => artifact.request_id === requestId)
            .map((artifact) => artifact.trial);
          const prepared = await client.prepareQualificationJob(requestId, completedTrials);
          if (!prepared.ok) return prepared;
          const queued = prepared.body as QualificationJob;
          try {
            await storeQualificationJob(queued);
            setQualificationJobState(queued);
          } catch {
            setQualificationExecutionArchiveError(true);
            return closureFailure('qualification_job_persistence_failed');
          }
          const dispatched = await client.dispatchQualificationJob(queued.job_id, requestId);
          if (dispatched.ok) {
            const running = dispatched.body as QualificationJob;
            setQualificationJobState(running);
            try {
              await storeQualificationJob(running);
            } catch {
              setQualificationExecutionArchiveError(true);
            }
          }
          return dispatched;
        }}
        onCancelQualification={async () => {
          const job = client.qualificationJob() ?? qualificationJobState;
          if (!job) return closureFailure('qualification_job_missing');
          const answer = await client.cancelQualificationJob(job.job_id, job.request_id);
          if (answer.ok) {
            const canceled = answer.body as QualificationJob;
            setQualificationJobState(canceled);
            try {
              await storeQualificationJob(canceled);
            } catch {
              setQualificationExecutionArchiveError(true);
            }
          }
          return answer;
        }}
        onResolveQualification={async () => {
          const job = client.qualificationJob() ?? qualificationJobState;
          if (!job || job.status !== 'completed') {
            return closureFailure('qualification_job_incomplete');
          }
          const artifacts = qualificationTrialArtifacts
            .filter((artifact) => artifact.job_id === job.job_id)
            .sort((left, right) => left.trial - right.trial);
          const answer = await client.resolveQualification(job.job_id, job.request_id, artifacts);
          if (!answer.ok) {
            const invalid = { ...job, status: 'invalid_execution' as const };
            setQualificationJobState(invalid);
            try {
              await storeQualificationJob(invalid);
            } catch {
              setQualificationExecutionArchiveError(true);
            }
            return answer;
          }
          const resolution = answer.body as QualificationResolution;
          try {
            for (const decision of resolution.criterion_decisions) {
              await storeQualificationCriterionDecision(decision);
            }
            await storeQualificationFunctionDecision(resolution.function_decision);
            setQualificationCriterionDecisionState(resolution.criterion_decisions);
            setQualificationFunctionDecisionState(resolution.function_decision);
            setQualificationExecutionArchiveError(false);
          } catch {
            setQualificationCriterionDecisionState(resolution.criterion_decisions);
            setQualificationFunctionDecisionState(resolution.function_decision);
            setQualificationExecutionArchiveError(true);
          }
          return answer;
        }}
        onGradeQualification={async () => {
          const job = client.qualificationJob() ?? qualificationJobState;
          const functionDecision = qualificationFunctionDecisionState;
          if (!job || !functionDecision) {
            return closureFailure('qualification_function_decision_missing');
          }
          const artifacts = qualificationTrialArtifacts
            .filter((artifact) => artifact.job_id === job.job_id)
            .sort((left, right) => left.trial - right.trial);
          const answer = await client.gradeQualification(
            job.job_id,
            job.request_id,
            functionDecision.function_decision_id,
            artifacts,
          );
          if (!answer.ok) return answer;
          const graded = answer.body as QualificationGrades;
          try {
            for (const grade of graded.grades) await storeQualificationGrade(grade);
            setQualificationGradeState(graded.grades);
            setQualificationExecutionArchiveError(false);
          } catch {
            setQualificationGradeState(graded.grades);
            setQualificationExecutionArchiveError(true);
          }
          return answer;
        }}
        onTraceQualificationFailure={async () => {
          const job = client.qualificationJob() ?? qualificationJobState;
          const functionDecision = qualificationFunctionDecisionState;
          if (!job || !functionDecision || functionDecision.definition.passed) {
            return closureFailure('qualification_failure_trace_not_applicable');
          }
          const artifacts = qualificationTrialArtifacts
            .filter((artifact) => artifact.job_id === job.job_id)
            .sort((left, right) => left.trial - right.trial);
          const answer = await client.traceQualificationFailure(
            job.job_id,
            job.request_id,
            functionDecision.function_decision_id,
            artifacts,
          );
          if (!answer.ok) return answer;
          const traced = answer.body as QualificationFailureTraceResult;
          if (!traced.failure_trace) return answer;
          try {
            await storeQualificationFailureTrace(traced.failure_trace);
            setQualificationFailureTraceState(traced.failure_trace);
            setQualificationExecutionArchiveError(false);
          } catch {
            setQualificationFailureTraceState(traced.failure_trace);
            setQualificationExecutionArchiveError(true);
          }
          return answer;
        }}
        onAssembleQualificationResult={async () => {
          const job = client.qualificationJob() ?? qualificationJobState;
          const functionDecision = qualificationFunctionDecisionState;
          if (!job || !functionDecision || qualificationGradeState.length !== 4) {
            return closureFailure('qualification_result_children_missing');
          }
          if (!functionDecision.definition.passed && !qualificationFailureTraceState) {
            return closureFailure('qualification_failure_trace_missing');
          }
          const artifacts = qualificationTrialArtifacts
            .filter((artifact) => artifact.job_id === job.job_id)
            .sort((left, right) => left.trial - right.trial);
          const axisOrder = ['throughput', 'resilience', 'economy', 'complexity'] as const;
          const grades = [...qualificationGradeState].sort((left, right) => (
            axisOrder.indexOf(left.definition.axis) - axisOrder.indexOf(right.definition.axis)
          ));
          const answer = await client.assembleQualificationResult(
            job.job_id,
            job.request_id,
            functionDecision.function_decision_id,
            grades.map((grade) => grade.grade_id),
            qualificationFailureTraceState?.failure_trace_id ?? null,
            artifacts,
          );
          if (!answer.ok) return answer;
          const group = answer.body as QualificationResultGroup;
          pendingQualificationResult.current = group;
          try {
            await storeQualificationResultGroup(group);
            setQualificationResultState(group);
            pendingQualificationResult.current = null;
            setQualificationExecutionArchiveError(false);
          } catch {
            setQualificationResultState(null);
            setQualificationExecutionArchiveError(true);
          }
          return answer;
        }}
        onProjectQualificationProgress={async () => {
          const group = qualificationResultState;
          const functionDecision = qualificationFunctionDecisionState;
          if (!group || !functionDecision || group.result.definition.outcome !== 'passed') {
            return closureFailure('qualification_result_not_eligible');
          }
          const axisOrder = ['throughput', 'resilience', 'economy', 'complexity'] as const;
          const grades = [...qualificationGradeState].sort((left, right) => (
            axisOrder.indexOf(left.definition.axis) - axisOrder.indexOf(right.definition.axis)
          ));
          const artifacts = qualificationTrialArtifacts
            .filter((artifact) => artifact.job_id === group.result.definition.job_id)
            .sort((left, right) => left.trial - right.trial);
          const answer = await client.deriveQualificationReceipt(
            group,
            functionDecision.function_decision_id,
            grades.map((grade) => grade.grade_id),
            qualificationFailureTraceState?.failure_trace_id ?? null,
            artifacts,
          );
          if (!answer.ok) return answer;
          const derived = answer.body as QualificationReceiptResult;
          pendingQualificationReceipt.current = derived.receipt;
          try {
            await storeQualificationUnlockReceipt(derived.receipt);
            const receipts = await qualificationUnlockReceipts();
            const catalog = await client.contracts(receipts);
            if (!('contract_version' in catalog)) {
              throw new Error('qualification_projection_refused');
            }
            setContractCatalog(catalog);
            setQualificationReceiptState(derived.receipt);
            pendingQualificationReceipt.current = null;
            setQualificationExecutionArchiveError(false);
          } catch {
            setQualificationReceiptState(null);
            setQualificationExecutionArchiveError(true);
          }
          return answer;
        }}
        onReadEngineeringAssembly={() => client.engineeringAssemblyDraft()}
        onPreviewEngineeringAssembly={(draft: EngineeringAssemblyDraft) => (
          client.previewEngineeringAssembly(draft)
        )}
        onAssemblyPreviewChange={(preview) => {
          setAssemblyPreview(preview);
          if (preview) setEngineeringTransition(null);
        }}
        onCommitEngineeringAssembly={async (
          draft: EngineeringAssemblyDraft,
          preview: EngineeringAssemblyPreview,
        ) => {
          const prepared = await prepareOrFault('superseded', 'assembly_revision');
          if (!prepared) return closureFailure('assembly_closure_prepare_failed');
          try {
            await prepareEngineeringAssemblyOperation(preview, {
              closure: prepared.record,
              exported: prepared.exported,
              identity: prepared.identity,
            });
          } catch {
            return closureFailure('assembly_operation_prepare_failed');
          }
          const answer = await client.commitEngineeringAssembly(draft, preview);
          if (!answer.ok) {
            await advanceEngineeringOperation(
              preview.preview_id,
              'refused',
              { error: answer.error.code },
            ).catch(() => undefined);
            return answer;
          }
          const commit = answer.body as EngineeringAssemblyCommitResult;
          const childIdentity = client.identity?.();
          let persistenceState: 'complete' | 'recovery_required' = 'complete';
          try {
            if (!childIdentity) throw new Error('assembly_child_identity_missing');
            await advanceEngineeringOperation(preview.preview_id, 'accepted_unpersisted', {
              acceptedCommit: commit,
              assemblyRecordId: commit.assembly_record.assembly_record_id,
              childIdentity,
              childAttemptId: commit.attempt_id,
              childBranchId: commit.branch_id,
              operationId: commit.transition_receipt.operation_id,
              recoveryState: commit.transition_receipt.recovery_state,
            });
            const exported = await client.command('export_run', {});
            if (!exported.ok) throw new Error('assembly_child_export_failed');
            const childExport = exported.body as RunExported;
            await advanceEngineeringOperation(preview.preview_id, 'accepted_unpersisted', {
              childExport,
            });
            const priorSave = await storeAutomationSessionSave(
              prepared.identity,
              prepared.exported,
            );
            await storeRunLineage(prepared.identity);
            await storeCommissionAttempt(prepared.record);
            await advanceEngineeringOperation(preview.preview_id, 'prior_retained', {
              priorSaveId: priorSave.id,
            });
            closedBranches.current.add(prepared.record.branchId);
            setCommissionHistory((held) => (
              held.some((entry) => entry.id === prepared.record.id)
                ? held
                : [prepared.record, ...held]
                  .sort((left, right) => right.recordedAt - left.recordedAt)
            ));
            await storeEngineeringAssemblyCommit(commit);
            await storeRunLineage(childIdentity);
            await advanceEngineeringOperation(preview.preview_id, 'child_published');
            const pointer = await publishEngineeringActiveSession(
              commit,
              childExport,
            );
            await advanceEngineeringOperation(preview.preview_id, 'pointer_moved', {
              pointerGeneration: pointer.pointerGeneration,
            });
            await advanceEngineeringOperation(preview.preview_id, 'complete', {
              recoveryState: 'persisted',
            });
            setCommissionArchiveError(false);
          } catch (cause: unknown) {
            persistenceState = 'recovery_required';
            await advanceEngineeringOperation(preview.preview_id, 'recovery_required', {
              error: cause instanceof Error ? cause.message : 'assembly_commit_persistence_failed',
            }).catch(() => undefined);
            setCommissionArchiveError(true);
          }
          try {
            const recoveries = await auditEngineeringOperations();
            setEngineeringRecoveries(recoveries);
            const operation = recoveries.find((outcome) => outcome.previewId === preview.preview_id);
            if (operation?.status === 'complete' || operation?.status === 'recovered') {
              persistenceState = 'complete';
              setCommissionArchiveError(false);
            } else if (operation?.status === 'manual_recovery') {
              persistenceState = 'recovery_required';
              setCommissionArchiveError(true);
            }
          } catch {
            persistenceState = 'recovery_required';
            setCommissionArchiveError(true);
          }
          setInspectionStep(null);
          setSelectedEventOrdinal(null);
          setPolicyPreview(null);
          setQualificationJobState(null);
          setQualificationTrialArtifacts([]);
          setQualificationCriterionDecisionState([]);
          setQualificationFunctionDecisionState(null);
          setQualificationGradeState([]);
          setQualificationFailureTraceState(null);
          setQualificationResultState(null);
          setQualificationReceiptState(null);
          return {
            ...answer,
            body: { ...answer.body, persistence_state: persistenceState },
          };
        }}
        onCaptureBlueprint={async (name) => {
          const source = qualificationResultState
            ? {
              kind: 'qualification_result' as const,
              result_id: qualificationResultState.result.result_id,
            }
            : { kind: 'committed_design' as const, result_id: null };
          const answer = await client.captureEngineeringMemory(source);
          if (!answer.ok) return answer;
          const capture = answer.body as EngineeringMemoryCapture;
          try {
            await storeEngineeringMemoryCapture(
              capture,
              name,
              captureBlueprintThumbnail(fieldCanvas.current, client.identity?.() ?? identity),
            );
            setBlueprints(await engineeringBlueprints());
            setQualificationExecutionArchiveError(false);
          } catch {
            setQualificationExecutionArchiveError(true);
          }
          return answer;
        }}
        onPreviewEngineeringTransition={(operation: EngineeringTransitionKind, entry) => {
          if (
            operation === 'revert_generator'
            && (!entry || entry.availability !== 'available' || !entry.generator)
          ) {
            return Promise.resolve(closureFailure('engineering_transition_source_unavailable'));
          }
          return client.previewEngineeringTransition(
            operation,
            operation === 'revert_generator' ? entry?.generator ?? undefined : undefined,
          );
        }}
        onTransitionPreviewChange={(preview) => {
          if (preview) setAssemblyPreview(null);
          setEngineeringTransition(preview ? {
            preview,
            receipt: null,
            status: 'preview',
          } : null);
        }}
        onCommitEngineeringTransition={async (preview: EngineeringRunTransitionPreview) => {
          if (!preview.definition.commit_allowed) {
            return closureFailure('engineering_transition_incompatible_preview');
          }
          const operation = preview.definition.operation;
          const closure: CommissionClosure = operation === 'restart_assembly'
            ? 'restart'
            : 'superseded';
          const boundary: CommissionRestartBoundary = operation === 'restart_assembly'
            ? 'committed_assembly'
            : operation === 'revert_generator'
              ? 'selected_generator'
              : 'authored_contract_opening';
          const prepared = await prepareOrFault(closure, boundary);
          if (!prepared) return closureFailure('engineering_transition_closure_prepare_failed');
          try {
            await prepareEngineeringTransitionOperation(preview, {
              closure: prepared.record,
              exported: prepared.exported,
              identity: prepared.identity,
            });
          } catch {
            return closureFailure('engineering_transition_operation_prepare_failed');
          }

          const answer = await client.commitEngineeringTransition(preview);
          if (!answer.ok) {
            await advanceEngineeringOperation(
              preview.preview_id,
              'refused',
              { error: answer.error.code },
            ).catch(() => undefined);
            return answer;
          }
          const body = answer.body as { code?: string; status?: string };
          if (body.status === 'refused') {
            await advanceEngineeringOperation(
              preview.preview_id,
              'refused',
              { error: body.code ?? 'engineering_transition_refused' },
            ).catch(() => undefined);
            return answer;
          }

          const commit = answer.body as EngineeringTransitionCommitResult;
          setEngineeringTransition({
            preview,
            receipt: commit.transition_receipt,
            status: 'committed',
          });
          const childIdentity = client.identity?.();
          let persistenceState: 'complete' | 'recovery_required' = 'complete';
          try {
            if (!childIdentity) throw new Error('engineering_transition_child_identity_missing');
            await advanceEngineeringOperation(preview.preview_id, 'accepted_unpersisted', {
              acceptedCommit: commit,
              assemblyRecordId: commit.assembly_record.assembly_record_id,
              childIdentity,
              childAttemptId: commit.attempt_id,
              childBranchId: commit.branch_id,
              operationId: commit.transition_receipt.operation_id,
              recoveryState: commit.transition_receipt.recovery_state,
            });
            const exported = await client.command('export_run', {});
            if (!exported.ok) throw new Error('engineering_transition_child_export_failed');
            const childExport = exported.body as RunExported;
            await advanceEngineeringOperation(preview.preview_id, 'accepted_unpersisted', {
              childExport,
            });
            const priorSave = await storeAutomationSessionSave(
              prepared.identity,
              prepared.exported,
            );
            await storeRunLineage(prepared.identity);
            await storeCommissionAttempt(prepared.record);
            await advanceEngineeringOperation(preview.preview_id, 'prior_retained', {
              priorSaveId: priorSave.id,
            });
            closedBranches.current.add(prepared.record.branchId);
            setCommissionHistory((held) => (
              held.some((entry) => entry.id === prepared.record.id)
                ? held
                : [prepared.record, ...held]
                  .sort((left, right) => right.recordedAt - left.recordedAt)
            ));
            await storeEngineeringTransitionCommit(commit);
            await storeRunLineage(childIdentity);
            await advanceEngineeringOperation(preview.preview_id, 'child_published');
            const pointer = await publishEngineeringActiveSession(commit, childExport);
            await advanceEngineeringOperation(preview.preview_id, 'pointer_moved', {
              pointerGeneration: pointer.pointerGeneration,
            });
            await advanceEngineeringOperation(preview.preview_id, 'complete', {
              recoveryState: 'persisted',
            });
            setCommissionArchiveError(false);
          } catch (cause: unknown) {
            persistenceState = 'recovery_required';
            await advanceEngineeringOperation(preview.preview_id, 'recovery_required', {
              error: cause instanceof Error
                ? cause.message
                : 'engineering_transition_persistence_failed',
            }).catch(() => undefined);
            setCommissionArchiveError(true);
          }
          try {
            const recoveries = await auditEngineeringOperations();
            setEngineeringRecoveries(recoveries);
            const resolved = recoveries.find((outcome) => (
              outcome.previewId === preview.preview_id
            ));
            if (resolved?.status === 'complete' || resolved?.status === 'recovered') {
              persistenceState = 'complete';
              setCommissionArchiveError(false);
            } else if (resolved?.status === 'manual_recovery') {
              persistenceState = 'recovery_required';
              setCommissionArchiveError(true);
            }
          } catch {
            persistenceState = 'recovery_required';
            setCommissionArchiveError(true);
          }
          setInspectionStep(null);
          setSelectedEventOrdinal(null);
          setPolicyPreview(null);
          setAssemblyPreview(null);
          setQualificationJobState(null);
          setQualificationTrialArtifacts([]);
          setQualificationCriterionDecisionState([]);
          setQualificationFunctionDecisionState(null);
          setQualificationGradeState([]);
          setQualificationFailureTraceState(null);
          setQualificationResultState(null);
          setQualificationReceiptState(null);
          return {
            ...answer,
            body: { ...answer.body, persistence_state: persistenceState },
          };
        }}
        onRestart={async (preview) => {
          const prepared = await prepareOrFault('restart', 'contract_opening');
          if (!prepared) return closureFailure('commission_closure_prepare_failed');
          const answer = await client.restartCommission(preview);
          if (!answer.ok) return answer;
          await persistCommissionClosure(prepared);
          setInspectionStep(null);
          setSelectedEventOrdinal(null);
          setPolicyPreview(null);
          return answer;
        }}
        onRate={(nextRate) => client.setRate(nextRate)}
        onTool={setTool}
        onUndo={() => { void client.undoPlan(); }}
        onCommit={async () => {
          const prepared = await prepareOrFault('superseded', 'current_embodied');
          if (prepared === false) return;
          const answer = await client.commitPlan();
          if (!answer.ok) return;
          const committed = answer.body as PlanCommitted;
          if (committed.applied > 0) {
            await persistCommissionClosure(prepared, {
              policyChanged: false,
              routeDefaultsChanged: [],
              topologyChanged: true,
            });
          }
        }}
        onDeployJunction={() => { void client.queuePlan({ op: 'deploy_junction' }); }}
        onOpenContracts={async () => {
          if (!qualificationResultState) {
            const paused = await client.setDesignMode(true);
            if ('code' in paused) return;
          }
          const prepared = await prepareOrFault('returned', null);
          if (prepared === false) return;
          const returned = await client.returnCommission();
          if (!returned.ok) return;
          await persistCommissionClosure(prepared);
          setInspectionStep(null);
          setSelectedEventOrdinal(null);
          setPolicyPreview(null);
          setContractsOpen(true);
        }}
        onOpenLab={() => setLabOpen(true)}
        onSetBreakpoint={(nextBreakpoint) => client.setCommissionBreakpoint(nextBreakpoint)}
        onPreviewPolicy={client.previewDesignPatch}
        onPreviewChange={setPolicyPreview}
        onSelectEvent={(entry) => {
          const addressed = mechanismAddress(entry);
          if (!addressed) return;
          setInspectionStep(entry.step);
          setSelectedEventOrdinal(entry.ordinal);
          void client.command('inspect_field', {
            id: addressed.id,
            step: entry.step,
            target: addressed.target,
          }).then((answer) => {
            if (answer.ok) {
              setSelection(answer.body as unknown as FieldInspection);
            } else {
              setInspectionStep(null);
              setSelectedEventOrdinal(null);
            }
          });
        }}
        onApplyPolicy={async (nextPolicy, nextRouteDefaults) => {
          const prepared = await prepareOrFault('superseded', 'current_embodied');
          if (prepared === false) return closureFailure('commission_closure_prepare_failed');
          const answer = await client.commitDesignPatch(nextPolicy, nextRouteDefaults);
          if (answer.ok) {
            const committed = answer.body as DesignCommitted;
            await persistCommissionClosure(prepared, {
              policyChanged: committed.canonical_diff.policy_changed,
              routeDefaultsChanged: [...committed.canonical_diff.route_defaults_changed],
              topologyChanged: false,
            });
          }
          if (answer.ok && selection && 'id' in selection) {
            const refreshed = await client.command('inspect_field', {
              id: selection.id,
              target: selection.target,
            });
            if (refreshed.ok) setSelection(refreshed.body as unknown as FieldInspection);
          }
          return answer;
        }}
      />
      {recovering ? <p className="notice">{copy('notice.run_resumed')}</p> : null}
      {labOpen && snapshot ? (
        <ExperimentLab
          client={client}
          frame={snapshot}
          regime={activeRegime}
          form={activeContract?.opening.form ?? chosen}
          view={view}
          onClose={() => setLabOpen(false)}
        />
      ) : null}
    </>
  );
}
