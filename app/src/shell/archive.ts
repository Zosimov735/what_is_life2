import type { FormId } from '../../../worker/src/protocol';
import type {
  CanonicalAttemptBranchRecord,
  CanonicalAttemptRecord,
  CanonicalQualificationRequest,
  CriterionReading,
  BlueprintRecord,
  ComparativeQualificationRecord,
  EngineeringAssemblyAdaptationRecord,
  EngineeringAssemblyCommitResult,
  EngineeringAssemblyDiff,
  EngineeringAssemblyPreview,
  EngineeringAssemblyRecord,
  EngineeringCompatibilityReport,
  EngineeringDiffReport,
  EngineeringGeneratorRecord,
  EngineeringMemoryCapture,
  EngineeringRunTransitionPreview,
  EngineeringTransitionCommitResult,
  EngineeringTransitionKind,
  EngineeringTransitionReceipt,
  QualificationCriterionDecision,
  QualificationFunctionDecision,
  QualificationGrade,
  QualificationFailureTrace,
  QualificationCompleteMarker,
  QualificationJob,
  QualificationResult,
  QualificationResultGroup,
  QualificationUnlockReceipt,
  QualificationTrialArtifact,
  RunExported,
  RunKind,
} from '../../../worker/src/protocol';
import { PROTOCOL_VERSION } from '../../../worker/src/protocol';
import type { RegimeId } from './Atlas';
import type { AnalysisScenario } from './experiment';
import type { MechanismTimelineEntry, RunIdentity } from './worker-client';
import { regimeById } from './regimes';

const DATABASE = 'what-is-life-archive';
const VERSION = 14;
const RUNS = 'runs';
const HOLDOUTS = 'holdouts';
const ATTEMPTS = 'attempts';
const ATTEMPT_BRANCHES = 'attempt_branches';
const COMMISSION_ATTEMPTS = 'commission_attempts';
const QUALIFICATION_REQUESTS = 'qualification_requests';
const QUALIFICATION_JOBS = 'qualification_jobs';
const QUALIFICATION_TRIALS = 'qualification_trials';
const QUALIFICATION_CRITERION_DECISIONS = 'qualification_criterion_decisions';
const QUALIFICATION_FUNCTION_DECISIONS = 'qualification_function_decisions';
const QUALIFICATION_GRADES = 'qualification_grades';
const QUALIFICATION_FAILURE_TRACES = 'qualification_failure_traces';
const QUALIFICATION_RESULTS = 'qualification_results';
const QUALIFICATION_RESULT_MARKERS = 'qualification_result_markers';
const QUALIFICATION_UNLOCK_RECEIPTS = 'qualification_unlock_receipts';
const ENGINEERING_ASSEMBLIES = 'engineering_assemblies';
const ENGINEERING_GENERATORS = 'engineering_generators';
const ENGINEERING_BLUEPRINTS = 'engineering_blueprints';
const ENGINEERING_BLUEPRINT_METADATA = 'engineering_blueprint_metadata';
const ENGINEERING_TRANSITIONS = 'engineering_transitions';
const ENGINEERING_DIFFS = 'engineering_diffs';
const ENGINEERING_COMPATIBILITY = 'engineering_compatibility';
const ENGINEERING_ADAPTATIONS = 'engineering_adaptations';
const ENGINEERING_COMPARATIVES = 'engineering_comparatives';
const ENGINEERING_MIGRATIONS = 'engineering_migrations';
const ENGINEERING_OPERATIONS = 'engineering_operations';
const AUTOMATION_SESSION_SAVES = 'automation_session_saves';
const ACTIVE_SESSION_POINTERS = 'active_session_pointers';
const ENGINEERING_V12_MIGRATION_ID = 'engineering:archive:12';
const ACTIVE_AUTOMATION_POINTER_ID = 'automation:active';
const ENGINEERING_AUTHORITY_STORES = [
  ENGINEERING_ASSEMBLIES,
  ENGINEERING_GENERATORS,
  ENGINEERING_BLUEPRINTS,
] as const;
const volatileRecords = new Map<string, ArchiveRecord>();
const volatileHoldouts = new Map<string, HoldoutSuite>();
const volatileAttempts = new Map<string, StoredAttemptRecord>();
const volatileAttemptBranches = new Map<string, StoredAttemptBranchRecord>();
const volatileCommissionAttempts = new Map<string, CommissionAttemptRecord>();
const volatileQualificationRequests = new Map<string, StoredQualificationRequest>();
const volatileQualificationJobs = new Map<string, StoredQualificationJob>();
const volatileQualificationTrials = new Map<string, StoredQualificationTrial>();
const volatileQualificationCriterionDecisions = new Map<string, StoredQualificationCriterionDecision>();
const volatileQualificationFunctionDecisions = new Map<string, StoredQualificationFunctionDecision>();
const volatileQualificationGrades = new Map<string, StoredQualificationGrade>();
const volatileQualificationFailureTraces = new Map<string, StoredQualificationFailureTrace>();
const volatileQualificationResults = new Map<string, StoredQualificationResult>();
const volatileQualificationResultMarkers = new Map<string, StoredQualificationCompleteMarker>();
const volatileQualificationUnlockReceipts = new Map<string, StoredQualificationUnlockReceipt>();
const volatileEngineeringAssemblies = new Map<string, StoredEngineeringAssembly>();
const volatileEngineeringGenerators = new Map<string, StoredEngineeringGenerator>();
const volatileEngineeringBlueprints = new Map<string, StoredEngineeringBlueprint>();
const volatileEngineeringBlueprintMetadata = new Map<string, EngineeringBlueprintMetadata>();
const volatileEngineeringTransitions = new Map<string, StoredEngineeringTransition>();
const volatileEngineeringDiffs = new Map<string, StoredEngineeringDiff>();
const volatileEngineeringCompatibility = new Map<string, StoredEngineeringCompatibility>();
const volatileEngineeringAdaptations = new Map<string, StoredEngineeringAdaptation>();
const volatileEngineeringComparatives = new Map<string, StoredEngineeringComparative>();
const volatileEngineeringMigrations = new Map<string, EngineeringMigrationJournal>();
const volatileEngineeringOperations = new Map<string, EngineeringOperationJournal>();
const volatileAutomationSessionSaves = new Map<string, StoredAutomationSessionSave>();
const volatileActiveSessionPointers = new Map<string, ActiveSessionPointer>();
let engineeringMigrationPromise: Promise<void> | null = null;

export type CommissionClosure = 'qualified' | 'restart' | 'returned' | 'superseded';
export type CommissionRestartBoundary =
  | 'contract_opening'
  | 'current_embodied'
  | 'assembly_revision'
  | 'committed_assembly'
  | 'selected_generator'
  | 'authored_contract_opening'
  | null;

export interface CommissionGeneratorDiff {
  policyChanged: boolean;
  routeDefaultsChanged: number[];
  topologyChanged: boolean;
}

export interface CommissionWeakestMargin {
  kind: 'component' | 'route' | 'leakage' | 'hands_off';
  objectId: number | null;
  margin: number;
  measured: number;
  required: number;
}

export interface StoredAttemptRecord {
  id: string;
  record: CanonicalAttemptRecord;
  storedAt: number;
}

export interface StoredAttemptBranchRecord {
  id: string;
  record: CanonicalAttemptBranchRecord;
  storedAt: number;
}

export interface StoredQualificationRequest {
  id: string;
  record: CanonicalQualificationRequest;
  storedAt: number;
}

export interface StoredQualificationJob {
  id: string;
  job: QualificationJob;
  updatedAt: number;
}

export interface StoredQualificationTrial {
  id: string;
  artifact: QualificationTrialArtifact;
  storedAt: number;
}

export interface StoredQualificationCriterionDecision {
  id: string;
  decision: QualificationCriterionDecision;
  storedAt: number;
}

export interface StoredQualificationFunctionDecision {
  id: string;
  decision: QualificationFunctionDecision;
  storedAt: number;
}

export interface StoredQualificationGrade {
  id: string;
  grade: QualificationGrade;
  storedAt: number;
}

export interface StoredQualificationFailureTrace {
  id: string;
  trace: QualificationFailureTrace;
  storedAt: number;
}

export interface StoredQualificationResult {
  id: string;
  result: QualificationResult;
  storedAt: number;
}

export interface StoredQualificationCompleteMarker {
  id: string;
  marker: QualificationCompleteMarker;
  storedAt: number;
}

export interface StoredQualificationResultGroup {
  completeMarker: StoredQualificationCompleteMarker;
  result: StoredQualificationResult;
}

export interface StoredQualificationUnlockReceipt {
  id: string;
  receipt: QualificationUnlockReceipt;
  storedAt: number;
}

export interface StoredEngineeringAssembly {
  id: string;
  record: EngineeringAssemblyRecord;
  storedAt: number;
}

export interface StoredEngineeringGenerator {
  id: string;
  record: EngineeringGeneratorRecord;
  storedAt: number;
}

export interface StoredEngineeringBlueprint {
  id: string;
  record: BlueprintRecord;
  storedAt: number;
}

export interface EngineeringBlueprintMetadata {
  id: string;
  blueprintId: string;
  name: string;
  tags: string[];
  thumbnail: EngineeringBlueprintThumbnail | null;
  updatedAt: number;
}

export interface EngineeringBlueprintThumbnail {
  assemblyHash: string;
  dataUrl: string;
  generatorHash: string;
  height: number;
  projectionVersion: 1;
  width: number;
}

export interface EngineeringBlueprintEntry {
  assembly: EngineeringAssemblyRecord | null;
  generator: EngineeringGeneratorRecord | null;
  metadata: EngineeringBlueprintMetadata;
  record: BlueprintRecord;
  unavailableRelationships: Array<'assembly' | 'generator'>;
}

export type EngineeringGeneratorSourceKind =
  | 'branch'
  | 'blueprint'
  | 'qualification_request'
  | 'qualification_result';

export type EngineeringGeneratorSourceAvailability =
  | 'available'
  | 'unavailable'
  | 'corrupt'
  | 'unsupported';

export type EngineeringGeneratorSourceReason =
  | 'available'
  | 'missing_ancestor_branch'
  | 'lineage_cycle'
  | 'attempt_mismatch'
  | 'missing_generator_record'
  | 'immutable_conflict'
  | 'unsupported_source_schema'
  | 'unsupported_generator_schema'
  | 'generator_hash_mismatch'
  | 'generator_source_mismatch'
  | 'contract_mismatch'
  | 'content_mismatch'
  | 'request_unavailable'
  | 'result_incomplete';

/** One exact retained source the Revert Generator picker can explain literally. */
export interface EngineeringGeneratorSourceEntry {
  ancestorDistance: number;
  attemptId: string;
  availability: EngineeringGeneratorSourceAvailability;
  branchId: string;
  branchOperation: CanonicalAttemptBranchRecord['operation'] | null;
  contentHash: string;
  contractId: string;
  generator: EngineeringGeneratorRecord | null;
  generatorHash: string;
  generatorRecordId: string | null;
  generatorSchema: number | null;
  id: string;
  kind: EngineeringGeneratorSourceKind;
  name: string | null;
  reason: EngineeringGeneratorSourceReason;
  resultOutcome: 'passed' | 'failed' | null;
  sourceId: string;
  sourceSchema: number;
}

export interface EngineeringGeneratorSourceContext {
  attemptId: string;
  branchId: string;
  contentHash: string;
  contractId: string;
  currentBranch: CanonicalAttemptBranchRecord;
}

function normalizedEngineeringMetadata(
  metadata: EngineeringBlueprintMetadata,
): EngineeringBlueprintMetadata {
  return { ...metadata, thumbnail: metadata.thumbnail ?? null };
}

export interface StoredEngineeringTransition {
  id: string;
  record: EngineeringTransitionReceipt;
  storedAt: number;
}

export interface StoredEngineeringDiff {
  id: string;
  record: EngineeringDiffReport | EngineeringAssemblyDiff;
  storedAt: number;
}

export interface StoredEngineeringCompatibility {
  id: string;
  record: EngineeringCompatibilityReport;
  storedAt: number;
}

export interface StoredEngineeringAdaptation {
  id: string;
  record: EngineeringAssemblyAdaptationRecord;
  storedAt: number;
}

export interface StoredEngineeringComparative {
  id: string;
  record: ComparativeQualificationRecord;
  storedAt: number;
}

export interface EngineeringMigrationJournal {
  id: typeof ENGINEERING_V12_MIGRATION_ID;
  conflictCount: number;
  conflictIds: string[];
  currentV2Count: number;
  error: string | null;
  fromVersion: number;
  lastKey: string | null;
  lastStore: string | null;
  phase: 'stores_created' | 'inventory' | 'v1_preserved';
  preservedV1Count: number;
  state: 'prepared' | 'migrating' | 'complete' | 'recovery_required';
  strategy: 'additive_preserve_v1';
  toVersion: 12;
  unavailableRelationshipCount: number;
  unavailableRelationshipIds: string[];
  unsupportedCount: number;
  unsupportedIds: string[];
  startedAt: number;
  updatedAt: number;
}

export type EngineeringOperationState =
  | 'prepared'
  | 'accepted_unpersisted'
  | 'prior_retained'
  | 'child_published'
  | 'pointer_moved'
  | 'complete'
  | 'refused'
  | 'recovery_required';

export interface EngineeringOperationJournal {
  acceptedCommit: EngineeringAssemblyCommitResult | EngineeringTransitionCommitResult | null;
  id: string;
  assemblyRecordId: string | null;
  childExport: RunExported | null;
  childIdentity: RunIdentity | null;
  childAttemptId: string | null;
  childBranchId: string | null;
  error: string | null;
  expectedAssemblyHash: string;
  expectedAttemptId: string;
  expectedBranchId: string;
  expectedContractId: string;
  expectedEmbodiedHash: string;
  expectedGeneratorHash: string;
  operation: 'assembly_commit' | EngineeringTransitionKind;
  operationId: string | null;
  pointerGeneration: number | null;
  previewId: string;
  priorClosure: CommissionAttemptRecord | null;
  priorExport: RunExported | null;
  priorIdentity: RunIdentity | null;
  priorSaveId: string | null;
  recoveryState: EngineeringTransitionReceipt['recovery_state'] | null;
  state: EngineeringOperationState;
  updatedAt: number;
  version: 2;
}

export interface EngineeringOperationRecovery {
  error: string | null;
  operation: EngineeringOperationJournal['operation'];
  operationId: string | null;
  previewId: string;
  state: EngineeringOperationState;
  status: 'complete' | 'refused' | 'prepared' | 'recovered' | 'manual_recovery';
}

export interface StoredAutomationSessionSave {
  id: string;
  assemblyHash: string;
  attemptId: string;
  branchId: string;
  contentHash: string;
  contractId: string;
  embodiedHash: string;
  generatorHash: string;
  payload: string;
  protocolVersion: number;
  runKind: 'automation_contract';
  storedAt: number;
  version: 1;
}

export interface ActiveSessionPointer {
  id: typeof ACTIVE_AUTOMATION_POINTER_ID;
  assemblyHash: string;
  attemptId: string;
  branchId: string;
  contentHash: string;
  contractId: string;
  generatorHash: string;
  operationId: string;
  pointerGeneration: number;
  protocolVersion: number;
  runKind: 'automation_contract';
  saveId: string;
  updatedAt: number;
  version: 1;
}

export interface CommissionAttemptRecord {
  id: string;
  schemaVersion: 1;
  contractId: string;
  attemptId: string;
  branchId: string;
  parentBranchId: string | null;
  branchNonce: number;
  branchOperation: CanonicalAttemptBranchRecord['operation'];
  runKind: RunKind;
  contentHash: string;
  generatorHash: string;
  assemblyHash: string;
  scenarioHash: string;
  regimeId: string;
  protocolVersion: number;
  closingEmbodiedHash: string;
  openingStep: number;
  closingStep: number;
  recordedAt: number;
  closure: CommissionClosure;
  restartBoundary: CommissionRestartBoundary;
  generatorDiff: CommissionGeneratorDiff | null;
  firstConsequenceOrdinal: number | null;
  weakestMargin: CommissionWeakestMargin | null;
  events: MechanismTimelineEntry[];
  criterion: CriterionReading | null;
}

export interface ArchiveRecord {
  id: string;
  schemaVersion: 2;
  engineBuildHash: string;
  lawsetVersion: string;
  protocolVersion: 2;
  contentHash: string;
  createdAt: number;
  runId: string;
  branchNonce: number;
  parentId: string | null;
  parent: { runId: string; branchNonce: number; anchorId: number | null } | null;
  scenarioId: string;
  scenarioHash: string;
  regime: RegimeId;
  form: FormId;
  embodiedStateHash: string;
  generatorHash: string;
  inputHash: string;
  controlHash: string;
  rngAlgorithm: 'Philox2x64-10';
  reproducibilityStateKey: string;
  estimatorVersion: string | null;
  analysisProtocolHash: string;
  trialCount: number;
  criterionVector: string[];
  payloadBlobKey: string;
  control: AnalysisScenario['control'];
  step: number;
  payload: string;
  evidence: Array<{
    kind: 'established' | 'withstood' | 'renewed' | 'paired_effect_observed' | 'transferred';
    passed: number;
    trials: number;
    artifact: string;
  }>;
}

function compactHash(value: string): string {
  let hash = 2_166_136_261;
  for (let place = 0; place < value.length; place += 1) {
    hash ^= value.charCodeAt(place);
    hash = Math.imul(hash, 16_777_619);
  }
  return (hash >>> 0).toString(16).padStart(8, '0');
}

/** Lifts pre-identity local records into the comparison contract in memory. */
function normalizeArchiveRecord(record: ArchiveRecord): ArchiveRecord {
  const legacy = record as ArchiveRecord & Partial<ArchiveRecord>;
  const payload = typeof legacy.payload === 'string' ? legacy.payload : '';
  const contentHash = legacy.contentHash || compactHash(payload);
  const scenarioHash = legacy.scenarioHash || legacy.scenarioId || contentHash;
  const generatorHash = legacy.generatorHash || contentHash;
  const embodiedStateHash = legacy.embodiedStateHash || compactHash(payload);
  const control = legacy.control || 'recorded_open_loop';
  return {
    ...legacy,
    schemaVersion: 2,
    protocolVersion: 2,
    contentHash,
    scenarioHash,
    generatorHash,
    embodiedStateHash,
    control,
    controlHash: legacy.controlHash || compactHash(JSON.stringify(control)),
    criterionVector: legacy.criterionVector ?? [],
    evidence: legacy.evidence ?? [],
  };
}

export interface HoldoutSuite {
  id: string;
  schemaVersion: 2;
  scenarioId: string;
  scenarioHash: string;
  embodiedStateHash: string;
  generatorHash: string;
  hiddenSuiteId: string;
  hiddenSuiteVersionHash: string;
  sealedBeforeCandidateHash: string;
  suiteSeed: number;
  createdAt: number;
  updatedAt: number;
  status: 'sealed' | 'evaluated' | 'contaminated' | 'retired';
  trials: number;
  requiredPasses: number;
  passed: number | null;
  contaminationReason: 'post_seal_scenario_change' | null;
}

function preparedEngineeringMigration(fromVersion: number): EngineeringMigrationJournal {
  const now = Date.now();
  return {
    id: ENGINEERING_V12_MIGRATION_ID,
    conflictCount: 0,
    conflictIds: [],
    currentV2Count: 0,
    error: null,
    fromVersion,
    lastKey: null,
    lastStore: null,
    phase: 'stores_created',
    preservedV1Count: 0,
    state: 'prepared',
    strategy: 'additive_preserve_v1',
    toVersion: 12,
    unavailableRelationshipCount: 0,
    unavailableRelationshipIds: [],
    unsupportedCount: 0,
    unsupportedIds: [],
    startedAt: now,
    updatedAt: now,
  };
}

type EngineeringAuthorityStore = typeof ENGINEERING_AUTHORITY_STORES[number];
type EngineeringAuthorityRow = { id: string; record: unknown };
type EngineeringAuthorityInventory = Record<EngineeringAuthorityStore, EngineeringAuthorityRow[]>;

function objectRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function authorityIdKey(store: EngineeringAuthorityStore): string {
  if (store === ENGINEERING_ASSEMBLIES) return 'assembly_record_id';
  if (store === ENGINEERING_GENERATORS) return 'generator_record_id';
  return 'blueprint_id';
}

function uniqueSorted(values: readonly string[]): string[] {
  return [...new Set(values)].sort((left, right) => left.localeCompare(right));
}

function completedEngineeringMigration(
  baseline: EngineeringMigrationJournal,
  inventory: EngineeringAuthorityInventory,
): EngineeringMigrationJournal {
  let preservedV1Count = 0;
  let currentV2Count = 0;
  const conflictIds: string[] = [];
  const unsupportedIds: string[] = [];
  const availableAssemblies = new Set(inventory[ENGINEERING_ASSEMBLIES].map((row) => row.id));
  const availableGenerators = new Set(inventory[ENGINEERING_GENERATORS].map((row) => row.id));
  const unavailableRelationshipIds: string[] = [];
  for (const store of ENGINEERING_AUTHORITY_STORES) {
    for (const row of inventory[store]) {
      const record = objectRecord(row.record);
      const definition = objectRecord(record?.definition);
      const declaredId = record?.[authorityIdKey(store)];
      if (typeof declaredId !== 'string' || declaredId !== row.id) {
        conflictIds.push(`${store}:${row.id}:identity`);
      }
      if (definition?.version === 1) preservedV1Count += 1;
      else if (definition?.version === 2) currentV2Count += 1;
      else unsupportedIds.push(`${store}:${row.id}`);
      if (store === ENGINEERING_BLUEPRINTS && definition) {
        const assemblyId = definition.assembly_record_id;
        const generatorId = definition.generator_record_id;
        if (typeof assemblyId !== 'string' || !availableAssemblies.has(assemblyId)) {
          unavailableRelationshipIds.push(`${row.id}:assembly`);
        }
        if (typeof generatorId !== 'string' || !availableGenerators.has(generatorId)) {
          unavailableRelationshipIds.push(`${row.id}:generator`);
        }
      }
    }
  }
  const conflicts = uniqueSorted(conflictIds);
  const unsupported = uniqueSorted(unsupportedIds);
  const unavailable = uniqueSorted(unavailableRelationshipIds);
  const lastStore = ENGINEERING_BLUEPRINTS;
  const lastKey = inventory[lastStore].at(-1)?.id ?? null;
  return {
    ...baseline,
    conflictCount: conflicts.length,
    conflictIds: conflicts,
    currentV2Count,
    error: conflicts.length > 0 ? 'immutable_record_identity_conflict' : null,
    lastKey,
    lastStore,
    phase: 'v1_preserved',
    preservedV1Count,
    state: conflicts.length > 0 ? 'recovery_required' : 'complete',
    unavailableRelationshipCount: unavailable.length,
    unavailableRelationshipIds: unavailable,
    unsupportedCount: unsupported.length,
    unsupportedIds: unsupported,
    updatedAt: Date.now(),
  };
}

function normalizeEngineeringMigration(value: unknown): EngineeringMigrationJournal {
  const found = objectRecord(value);
  const prepared = preparedEngineeringMigration(
    typeof found?.fromVersion === 'number' ? found.fromVersion : VERSION,
  );
  return {
    ...prepared,
    ...(found ?? {}),
    id: ENGINEERING_V12_MIGRATION_ID,
    conflictCount: typeof found?.conflictCount === 'number' ? found.conflictCount : 0,
    conflictIds: Array.isArray(found?.conflictIds)
      ? found.conflictIds.filter((item): item is string => typeof item === 'string')
      : [],
    currentV2Count: typeof found?.currentV2Count === 'number' ? found.currentV2Count : 0,
    lastStore: typeof found?.lastStore === 'string' ? found.lastStore : null,
    preservedV1Count: typeof found?.preservedV1Count === 'number' ? found.preservedV1Count : 0,
    toVersion: 12,
    unavailableRelationshipCount: typeof found?.unavailableRelationshipCount === 'number'
      ? found.unavailableRelationshipCount
      : 0,
    unavailableRelationshipIds: Array.isArray(found?.unavailableRelationshipIds)
      ? found.unavailableRelationshipIds.filter((item): item is string => typeof item === 'string')
      : [],
    unsupportedCount: typeof found?.unsupportedCount === 'number' ? found.unsupportedCount : 0,
    unsupportedIds: Array.isArray(found?.unsupportedIds)
      ? found.unsupportedIds.filter((item): item is string => typeof item === 'string')
      : [],
  } as EngineeringMigrationJournal;
}

function hasEngineeringInventory(value: unknown): boolean {
  const found = objectRecord(value);
  return typeof found?.conflictCount === 'number'
    && Array.isArray(found.conflictIds)
    && typeof found.currentV2Count === 'number'
    && typeof found.preservedV1Count === 'number'
    && typeof found.unavailableRelationshipCount === 'number'
    && typeof found.unsupportedCount === 'number';
}

async function readEngineeringMigration(
  database: IDBDatabase,
): Promise<EngineeringMigrationJournal | null> {
  const transaction = database.transaction(ENGINEERING_MIGRATIONS, 'readonly');
  const done = transactionDone(transaction);
  const result = await requestResult(
    transaction.objectStore(ENGINEERING_MIGRATIONS).get(ENGINEERING_V12_MIGRATION_ID),
  );
  await done;
  return result ? normalizeEngineeringMigration(result) : null;
}

async function writeEngineeringMigration(
  database: IDBDatabase,
  journal: EngineeringMigrationJournal,
): Promise<void> {
  const transaction = database.transaction(ENGINEERING_MIGRATIONS, 'readwrite');
  const done = transactionDone(transaction);
  await requestResult(transaction.objectStore(ENGINEERING_MIGRATIONS).put(journal));
  await done;
  volatileEngineeringMigrations.set(journal.id, journal);
}

async function engineeringAuthorityInventory(
  database: IDBDatabase,
): Promise<EngineeringAuthorityInventory> {
  const transaction = database.transaction([...ENGINEERING_AUTHORITY_STORES], 'readonly');
  const done = transactionDone(transaction);
  const [assemblies, generators, blueprints] = await Promise.all([
    requestResult(transaction.objectStore(ENGINEERING_ASSEMBLIES).getAll()),
    requestResult(transaction.objectStore(ENGINEERING_GENERATORS).getAll()),
    requestResult(transaction.objectStore(ENGINEERING_BLUEPRINTS).getAll()),
  ]) as [EngineeringAuthorityRow[], EngineeringAuthorityRow[], EngineeringAuthorityRow[]];
  await done;
  const ordered = (rows: EngineeringAuthorityRow[]): EngineeringAuthorityRow[] => (
    rows.sort((left, right) => left.id.localeCompare(right.id))
  );
  return {
    [ENGINEERING_ASSEMBLIES]: ordered(assemblies),
    [ENGINEERING_GENERATORS]: ordered(generators),
    [ENGINEERING_BLUEPRINTS]: ordered(blueprints),
  };
}

async function runEngineeringMigration(database: IDBDatabase): Promise<void> {
  const currentRaw = await readEngineeringMigration(database);
  const current = currentRaw ?? preparedEngineeringMigration(VERSION);
  if (
    currentRaw
    && hasEngineeringInventory(currentRaw)
    && (current.state === 'complete' || current.state === 'recovery_required')
  ) return;
  if (!currentRaw) await writeEngineeringMigration(database, current);
  const inventoryState: EngineeringMigrationJournal = {
    ...current,
    error: null,
    lastKey: null,
    lastStore: null,
    phase: 'inventory',
    state: 'migrating',
    updatedAt: Date.now(),
  };
  await writeEngineeringMigration(database, inventoryState);
  const inventory = await engineeringAuthorityInventory(database);
  await writeEngineeringMigration(
    database,
    completedEngineeringMigration(inventoryState, inventory),
  );
}

async function ensureEngineeringMigrationJournal(database: IDBDatabase): Promise<void> {
  if (!engineeringMigrationPromise) {
    engineeringMigrationPromise = runEngineeringMigration(database).catch(async (cause: unknown) => {
      const prior = await readEngineeringMigration(database).catch(() => null)
        ?? preparedEngineeringMigration(VERSION);
      const recovery: EngineeringMigrationJournal = {
        ...prior,
        error: cause instanceof Error ? cause.message : 'engineering_migration_failed',
        state: 'recovery_required',
        updatedAt: Date.now(),
      };
      volatileEngineeringMigrations.set(recovery.id, recovery);
      await writeEngineeringMigration(database, recovery).catch(() => undefined);
    });
  }
  await engineeringMigrationPromise;
}

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE, VERSION);
    request.onupgradeneeded = (event) => {
      const database = request.result;
      if (!database.objectStoreNames.contains(RUNS)) {
        const store = database.createObjectStore(RUNS, { keyPath: 'id' });
        store.createIndex('createdAt', 'createdAt');
        store.createIndex('scenarioId', 'scenarioId');
      }
      if (!database.objectStoreNames.contains(HOLDOUTS)) {
        const store = database.createObjectStore(HOLDOUTS, { keyPath: 'id' });
        store.createIndex('createdAt', 'createdAt');
        store.createIndex('scenarioId', 'scenarioId');
        store.createIndex('status', 'status');
      }
      if (!database.objectStoreNames.contains(ATTEMPTS)) {
        const store = database.createObjectStore(ATTEMPTS, { keyPath: 'id' });
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(ATTEMPT_BRANCHES)) {
        const store = database.createObjectStore(ATTEMPT_BRANCHES, { keyPath: 'id' });
        store.createIndex('storedAt', 'storedAt');
        store.createIndex('attemptId', 'record.attempt_id');
        store.createIndex('parentBranchId', 'record.parent_branch_id');
      }
      if (!database.objectStoreNames.contains(COMMISSION_ATTEMPTS)) {
        const store = database.createObjectStore(COMMISSION_ATTEMPTS, { keyPath: 'id' });
        store.createIndex('contractId', 'contractId');
        store.createIndex('attemptId', 'attemptId');
        store.createIndex('recordedAt', 'recordedAt');
      }
      if (!database.objectStoreNames.contains(QUALIFICATION_REQUESTS)) {
        const store = database.createObjectStore(QUALIFICATION_REQUESTS, { keyPath: 'id' });
        store.createIndex('attemptId', 'record.input.attempt_id');
        store.createIndex('branchId', 'record.input.branch_id');
        store.createIndex('contractId', 'record.input.contract_id');
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(QUALIFICATION_JOBS)) {
        const store = database.createObjectStore(QUALIFICATION_JOBS, { keyPath: 'id' });
        store.createIndex('requestId', 'job.request_id');
        store.createIndex('status', 'job.status');
        store.createIndex('updatedAt', 'updatedAt');
      }
      if (!database.objectStoreNames.contains(QUALIFICATION_TRIALS)) {
        const store = database.createObjectStore(QUALIFICATION_TRIALS, { keyPath: 'id' });
        store.createIndex('jobId', 'artifact.job_id');
        store.createIndex('requestId', 'artifact.request_id');
        store.createIndex('trial', 'artifact.trial');
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(QUALIFICATION_CRITERION_DECISIONS)) {
        const store = database.createObjectStore(QUALIFICATION_CRITERION_DECISIONS, { keyPath: 'id' });
        store.createIndex('artifactId', 'decision.definition.artifact_id');
        store.createIndex('jobId', 'decision.definition.job_id');
        store.createIndex('requestId', 'decision.definition.request_id');
        store.createIndex('trial', 'decision.definition.trial');
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(QUALIFICATION_FUNCTION_DECISIONS)) {
        const store = database.createObjectStore(QUALIFICATION_FUNCTION_DECISIONS, { keyPath: 'id' });
        store.createIndex('jobId', 'decision.definition.job_id', { unique: true });
        store.createIndex('requestId', 'decision.definition.request_id');
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(QUALIFICATION_GRADES)) {
        const store = database.createObjectStore(QUALIFICATION_GRADES, { keyPath: 'id' });
        store.createIndex('axis', 'grade.definition.axis');
        store.createIndex('functionDecisionId', 'grade.definition.function_decision_id');
        store.createIndex('jobId', 'grade.definition.job_id');
        store.createIndex('requestId', 'grade.definition.request_id');
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(QUALIFICATION_FAILURE_TRACES)) {
        const store = database.createObjectStore(QUALIFICATION_FAILURE_TRACES, { keyPath: 'id' });
        store.createIndex('criterionDecisionId', 'trace.definition.criterion_decision_id');
        store.createIndex('functionDecisionId', 'trace.definition.function_decision_id');
        store.createIndex('jobId', 'trace.definition.job_id');
        store.createIndex('requestId', 'trace.definition.request_id');
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(QUALIFICATION_RESULTS)) {
        const store = database.createObjectStore(QUALIFICATION_RESULTS, { keyPath: 'id' });
        store.createIndex('contractId', 'result.definition.contract_id');
        store.createIndex('jobId', 'result.definition.job_id', { unique: true });
        store.createIndex('outcome', 'result.definition.outcome');
        store.createIndex('requestId', 'result.definition.request_id', { unique: true });
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(QUALIFICATION_RESULT_MARKERS)) {
        const store = database.createObjectStore(QUALIFICATION_RESULT_MARKERS, { keyPath: 'id' });
        store.createIndex('resultId', 'marker.definition.result_id', { unique: true });
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(QUALIFICATION_UNLOCK_RECEIPTS)) {
        const store = database.createObjectStore(QUALIFICATION_UNLOCK_RECEIPTS, { keyPath: 'id' });
        store.createIndex('contractId', 'receipt.definition.contract_id');
        store.createIndex('resultId', 'receipt.definition.result_id', { unique: true });
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(ENGINEERING_ASSEMBLIES)) {
        const store = database.createObjectStore(ENGINEERING_ASSEMBLIES, { keyPath: 'id' });
        store.createIndex('assemblyHash', 'record.definition.assembly_template_hash');
        store.createIndex('contractId', 'record.definition.compatibility.contract_id');
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(ENGINEERING_GENERATORS)) {
        const store = database.createObjectStore(ENGINEERING_GENERATORS, { keyPath: 'id' });
        store.createIndex('generatorHash', 'record.definition.generator_spec_hash');
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(ENGINEERING_BLUEPRINTS)) {
        const store = database.createObjectStore(ENGINEERING_BLUEPRINTS, { keyPath: 'id' });
        store.createIndex('assemblyRecordId', 'record.definition.assembly_record_id');
        store.createIndex('contractId', 'record.definition.contract_id');
        store.createIndex('generatorRecordId', 'record.definition.generator_record_id');
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(ENGINEERING_BLUEPRINT_METADATA)) {
        const store = database.createObjectStore(ENGINEERING_BLUEPRINT_METADATA, { keyPath: 'id' });
        store.createIndex('blueprintId', 'blueprintId', { unique: true });
        store.createIndex('updatedAt', 'updatedAt');
      }
      if (!database.objectStoreNames.contains(ENGINEERING_TRANSITIONS)) {
        const store = database.createObjectStore(ENGINEERING_TRANSITIONS, { keyPath: 'id' });
        store.createIndex('operation', 'record.operation');
        store.createIndex('parentBranchId', 'record.parent_branch_id');
        store.createIndex('childBranchId', 'record.child_branch_id');
        store.createIndex('previewId', 'record.preview_id');
        store.createIndex('storedAt', 'storedAt');
      } else {
        const store = request.transaction?.objectStore(ENGINEERING_TRANSITIONS);
        if (store && !store.indexNames.contains('previewId')) {
          store.createIndex('previewId', 'record.preview_id');
        }
      }
      if (!database.objectStoreNames.contains(ENGINEERING_DIFFS)) {
        const store = database.createObjectStore(ENGINEERING_DIFFS, { keyPath: 'id' });
        store.createIndex('leftId', 'record.definition.left_id');
        store.createIndex('rightId', 'record.definition.right_id');
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(ENGINEERING_COMPATIBILITY)) {
        const store = database.createObjectStore(ENGINEERING_COMPATIBILITY, { keyPath: 'id' });
        store.createIndex('sourceGeneratorId', 'record.definition.source_generator_record_id');
        store.createIndex('destinationContractId', 'record.definition.destination_contract_id');
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(ENGINEERING_ADAPTATIONS)) {
        const store = database.createObjectStore(ENGINEERING_ADAPTATIONS, { keyPath: 'id' });
        store.createIndex('compatibilityId', 'record.definition.compatibility_id');
        store.createIndex('destinationBranchId', 'record.definition.destination_branch_id');
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(ENGINEERING_COMPARATIVES)) {
        const store = database.createObjectStore(ENGINEERING_COMPARATIVES, { keyPath: 'id' });
        store.createIndex('sourceResultId', 'record.definition.source_result_id');
        store.createIndex('destinationResultId', 'record.definition.destination_result_id');
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(ENGINEERING_MIGRATIONS)) {
        const store = database.createObjectStore(ENGINEERING_MIGRATIONS, { keyPath: 'id' });
        store.createIndex('state', 'state');
        store.createIndex('updatedAt', 'updatedAt');
      }
      if (!database.objectStoreNames.contains(ENGINEERING_OPERATIONS)) {
        const store = database.createObjectStore(ENGINEERING_OPERATIONS, { keyPath: 'id' });
        store.createIndex('operationId', 'operationId');
        store.createIndex('previewId', 'previewId', { unique: true });
        store.createIndex('state', 'state');
        store.createIndex('updatedAt', 'updatedAt');
      }
      if (!database.objectStoreNames.contains(AUTOMATION_SESSION_SAVES)) {
        const store = database.createObjectStore(AUTOMATION_SESSION_SAVES, { keyPath: 'id' });
        store.createIndex('attemptId', 'attemptId');
        store.createIndex('branchId', 'branchId');
        store.createIndex('contractId', 'contractId');
        store.createIndex('storedAt', 'storedAt');
      }
      if (!database.objectStoreNames.contains(ACTIVE_SESSION_POINTERS)) {
        const store = database.createObjectStore(ACTIVE_SESSION_POINTERS, { keyPath: 'id' });
        store.createIndex('updatedAt', 'updatedAt');
      }
      const fromVersion = (event as IDBVersionChangeEvent).oldVersion;
      request.transaction?.objectStore(ENGINEERING_MIGRATIONS).put(
        preparedEngineeringMigration(fromVersion),
      );
    };
    request.onsuccess = () => {
      const database = request.result;
      void ensureEngineeringMigrationJournal(database).then(
        () => resolve(database),
        (cause) => {
          database.close();
          reject(cause);
        },
      );
    };
    request.onerror = () => reject(request.error ?? new Error('archive_open_failed'));
  });
}

export async function storeRunLineage(identity: RunIdentity): Promise<void> {
  const attempt = identity.attemptRecord;
  const branch = identity.attemptBranch;
  if (!attempt || !branch) return;
  const now = Date.now();
  const attemptRow: StoredAttemptRecord = {
    id: attempt.attempt_id,
    record: attempt,
    storedAt: now,
  };
  const branchRow: StoredAttemptBranchRecord = {
    id: branch.branch_id,
    record: branch,
    storedAt: now,
  };
  if (typeof indexedDB === 'undefined') {
    addVolatileImmutable(volatileAttempts, attemptRow, (left, right) => (
      sameValue(left.record, right.record)
    ));
    addVolatileImmutable(volatileAttemptBranches, branchRow, (left, right) => (
      sameValue(left.record, right.record)
    ));
    return;
  }
  try {
    await Promise.all([
      addIndexedImmutable(ATTEMPTS, attemptRow, (left, right) => (
        sameValue(left.record, right.record)
      )),
      addIndexedImmutable(ATTEMPT_BRANCHES, branchRow, (left, right) => (
        sameValue(left.record, right.record)
      )),
    ]);
  } catch (cause) {
    addVolatileImmutable(volatileAttempts, attemptRow, (left, right) => (
      sameValue(left.record, right.record)
    ));
    addVolatileImmutable(volatileAttemptBranches, branchRow, (left, right) => (
      sameValue(left.record, right.record)
    ));
    throw cause;
  }
}

export async function storeCommissionAttempt(record: CommissionAttemptRecord): Promise<void> {
  if (typeof indexedDB === 'undefined') {
    addVolatileImmutable(volatileCommissionAttempts, record, sameCommissionAttempt);
    return;
  }
  try {
    await addIndexedImmutable(COMMISSION_ATTEMPTS, record, sameCommissionAttempt);
  } catch (cause) {
    addVolatileImmutable(volatileCommissionAttempts, record, sameCommissionAttempt);
    throw cause;
  }
}

export async function storeQualificationRequest(
  record: CanonicalQualificationRequest,
): Promise<void> {
  const row: StoredQualificationRequest = {
    id: record.request_id,
    record,
    storedAt: Date.now(),
  };
  const equivalent = (
    left: StoredQualificationRequest,
    right: StoredQualificationRequest,
  ): boolean => sameValue(left.record, right.record);
  if (typeof indexedDB === 'undefined') {
    addVolatileImmutable(volatileQualificationRequests, row, equivalent);
    return;
  }
  try {
    await addIndexedImmutable(QUALIFICATION_REQUESTS, row, equivalent);
  } catch (cause) {
    addVolatileImmutable(volatileQualificationRequests, row, equivalent);
    throw cause;
  }
}

export async function qualificationRequests(
  contractId?: string,
): Promise<StoredQualificationRequest[]> {
  if (typeof indexedDB === 'undefined') {
    return [...volatileQualificationRequests.values()]
      .filter((row) => !contractId || row.record.input.contract_id === contractId)
      .sort((left, right) => right.storedAt - left.storedAt);
  }
  const database = await openDatabase();
  try {
    const store = database.transaction(QUALIFICATION_REQUESTS, 'readonly')
      .objectStore(QUALIFICATION_REQUESTS);
    const rows = contractId
      ? await requestResult(store.index('contractId').getAll(contractId)) as StoredQualificationRequest[]
      : await requestResult(store.getAll()) as StoredQualificationRequest[];
    const merged = new Map(rows.map((row) => [row.id, row]));
    for (const row of volatileQualificationRequests.values()) {
      if (!contractId || row.record.input.contract_id === contractId) merged.set(row.id, row);
    }
    return [...merged.values()].sort((left, right) => right.storedAt - left.storedAt);
  } finally {
    database.close();
  }
}

function sameQualificationJobAuthority(
  left: StoredQualificationJob,
  right: StoredQualificationJob,
): boolean {
  return left.job.job_id === right.job.job_id
    && left.job.request_id === right.job.request_id
    && left.job.duration_steps === right.job.duration_steps
    && left.job.progress_interval_steps === right.job.progress_interval_steps
    && left.job.trial_count === right.job.trial_count
    && left.job.version === right.job.version;
}

export async function storeQualificationJob(job: QualificationJob): Promise<void> {
  const row: StoredQualificationJob = { id: job.job_id, job, updatedAt: Date.now() };
  const volatile = volatileQualificationJobs.get(row.id);
  if (volatile && !sameQualificationJobAuthority(volatile, row)) {
    throw immutableConflict(row.id);
  }
  volatileQualificationJobs.set(row.id, row);
  if (typeof indexedDB === 'undefined') return;
  const existing = await indexedRow<StoredQualificationJob>(QUALIFICATION_JOBS, row.id);
  if (existing && !sameQualificationJobAuthority(existing, row)) {
    throw immutableConflict(row.id);
  }
  const database = await openDatabase();
  try {
    const transaction = database.transaction(QUALIFICATION_JOBS, 'readwrite');
    await requestResult(transaction.objectStore(QUALIFICATION_JOBS).put(row));
  } finally {
    database.close();
  }
}

export async function qualificationJob(
  requestId: string,
): Promise<StoredQualificationJob | null> {
  const volatile = [...volatileQualificationJobs.values()]
    .filter((row) => row.job.request_id === requestId)
    .sort((left, right) => right.updatedAt - left.updatedAt)[0] ?? null;
  if (typeof indexedDB === 'undefined') return volatile;
  const database = await openDatabase();
  try {
    const rows = await requestResult(
      database.transaction(QUALIFICATION_JOBS, 'readonly')
        .objectStore(QUALIFICATION_JOBS)
        .index('requestId')
        .getAll(requestId),
    ) as StoredQualificationJob[];
    if (volatile) rows.push(volatile);
    return rows.sort((left, right) => right.updatedAt - left.updatedAt)[0] ?? null;
  } finally {
    database.close();
  }
}

export async function storeQualificationTrial(
  artifact: QualificationTrialArtifact,
): Promise<void> {
  const row: StoredQualificationTrial = {
    id: artifact.artifact_id,
    artifact,
    storedAt: Date.now(),
  };
  const equivalent = (
    left: StoredQualificationTrial,
    right: StoredQualificationTrial,
  ): boolean => sameValue(left.artifact, right.artifact);
  if (typeof indexedDB === 'undefined') {
    addVolatileImmutable(volatileQualificationTrials, row, equivalent);
    return;
  }
  try {
    await addIndexedImmutable(QUALIFICATION_TRIALS, row, equivalent);
  } catch (cause) {
    addVolatileImmutable(volatileQualificationTrials, row, equivalent);
    throw cause;
  }
}

export async function qualificationTrials(
  jobId: string,
): Promise<StoredQualificationTrial[]> {
  if (typeof indexedDB === 'undefined') {
    return [...volatileQualificationTrials.values()]
      .filter((row) => row.artifact.job_id === jobId)
      .sort((left, right) => left.artifact.trial - right.artifact.trial);
  }
  const database = await openDatabase();
  try {
    const rows = await requestResult(
      database.transaction(QUALIFICATION_TRIALS, 'readonly')
        .objectStore(QUALIFICATION_TRIALS)
        .index('jobId')
        .getAll(jobId),
    ) as StoredQualificationTrial[];
    const merged = new Map(rows.map((row) => [row.id, row]));
    for (const row of volatileQualificationTrials.values()) {
      if (row.artifact.job_id === jobId) merged.set(row.id, row);
    }
    return [...merged.values()]
      .sort((left, right) => left.artifact.trial - right.artifact.trial);
  } finally {
    database.close();
  }
}

export async function storeQualificationCriterionDecision(
  decision: QualificationCriterionDecision,
): Promise<void> {
  const row: StoredQualificationCriterionDecision = {
    id: decision.decision_id,
    decision,
    storedAt: Date.now(),
  };
  const equivalent = (
    left: StoredQualificationCriterionDecision,
    right: StoredQualificationCriterionDecision,
  ): boolean => sameValue(left.decision, right.decision);
  if (typeof indexedDB === 'undefined') {
    addVolatileImmutable(volatileQualificationCriterionDecisions, row, equivalent);
    return;
  }
  try {
    await addIndexedImmutable(QUALIFICATION_CRITERION_DECISIONS, row, equivalent);
  } catch (cause) {
    addVolatileImmutable(volatileQualificationCriterionDecisions, row, equivalent);
    throw cause;
  }
}

export async function qualificationCriterionDecisions(
  jobId: string,
): Promise<StoredQualificationCriterionDecision[]> {
  if (typeof indexedDB === 'undefined') {
    return [...volatileQualificationCriterionDecisions.values()]
      .filter((row) => row.decision.definition.job_id === jobId)
      .sort((left, right) => (
        left.decision.definition.trial - right.decision.definition.trial
        || left.decision.definition.criterion_id.localeCompare(right.decision.definition.criterion_id)
      ));
  }
  const database = await openDatabase();
  try {
    const rows = await requestResult(
      database.transaction(QUALIFICATION_CRITERION_DECISIONS, 'readonly')
        .objectStore(QUALIFICATION_CRITERION_DECISIONS)
        .index('jobId')
        .getAll(jobId),
    ) as StoredQualificationCriterionDecision[];
    const merged = new Map(rows.map((row) => [row.id, row]));
    for (const row of volatileQualificationCriterionDecisions.values()) {
      if (row.decision.definition.job_id === jobId) merged.set(row.id, row);
    }
    return [...merged.values()].sort((left, right) => (
      left.decision.definition.trial - right.decision.definition.trial
      || left.decision.definition.criterion_id.localeCompare(right.decision.definition.criterion_id)
    ));
  } finally {
    database.close();
  }
}

export async function storeQualificationFunctionDecision(
  decision: QualificationFunctionDecision,
): Promise<void> {
  const row: StoredQualificationFunctionDecision = {
    id: decision.function_decision_id,
    decision,
    storedAt: Date.now(),
  };
  const equivalent = (
    left: StoredQualificationFunctionDecision,
    right: StoredQualificationFunctionDecision,
  ): boolean => sameValue(left.decision, right.decision);
  if (typeof indexedDB === 'undefined') {
    addVolatileImmutable(volatileQualificationFunctionDecisions, row, equivalent);
    return;
  }
  try {
    await addIndexedImmutable(QUALIFICATION_FUNCTION_DECISIONS, row, equivalent);
  } catch (cause) {
    addVolatileImmutable(volatileQualificationFunctionDecisions, row, equivalent);
    throw cause;
  }
}

export async function qualificationFunctionDecision(
  jobId: string,
): Promise<StoredQualificationFunctionDecision | null> {
  const volatile = [...volatileQualificationFunctionDecisions.values()]
    .find((row) => row.decision.definition.job_id === jobId) ?? null;
  if (typeof indexedDB === 'undefined') return volatile;
  const database = await openDatabase();
  try {
    const row = await requestResult(
      database.transaction(QUALIFICATION_FUNCTION_DECISIONS, 'readonly')
        .objectStore(QUALIFICATION_FUNCTION_DECISIONS)
        .index('jobId')
        .get(jobId),
    ) as StoredQualificationFunctionDecision | undefined;
    if (row && volatile && !sameValue(row.decision, volatile.decision)) {
      throw immutableConflict(jobId);
    }
    return row ?? volatile;
  } finally {
    database.close();
  }
}

export async function storeQualificationGrade(grade: QualificationGrade): Promise<void> {
  const row: StoredQualificationGrade = {
    id: grade.grade_id,
    grade,
    storedAt: Date.now(),
  };
  const equivalent = (
    left: StoredQualificationGrade,
    right: StoredQualificationGrade,
  ): boolean => sameValue(left.grade, right.grade);
  if (typeof indexedDB === 'undefined') {
    addVolatileImmutable(volatileQualificationGrades, row, equivalent);
    return;
  }
  try {
    await addIndexedImmutable(QUALIFICATION_GRADES, row, equivalent);
  } catch (cause) {
    addVolatileImmutable(volatileQualificationGrades, row, equivalent);
    throw cause;
  }
}

export async function qualificationGrades(jobId: string): Promise<StoredQualificationGrade[]> {
  if (typeof indexedDB === 'undefined') {
    return [...volatileQualificationGrades.values()]
      .filter((row) => row.grade.definition.job_id === jobId)
      .sort((left, right) => left.grade.definition.axis.localeCompare(right.grade.definition.axis));
  }
  const database = await openDatabase();
  try {
    const rows = await requestResult(
      database.transaction(QUALIFICATION_GRADES, 'readonly')
        .objectStore(QUALIFICATION_GRADES)
        .index('jobId')
        .getAll(jobId),
    ) as StoredQualificationGrade[];
    const merged = new Map(rows.map((row) => [row.id, row]));
    for (const row of volatileQualificationGrades.values()) {
      if (row.grade.definition.job_id === jobId) merged.set(row.id, row);
    }
    return [...merged.values()]
      .sort((left, right) => left.grade.definition.axis.localeCompare(right.grade.definition.axis));
  } finally {
    database.close();
  }
}

export async function storeQualificationFailureTrace(
  trace: QualificationFailureTrace,
): Promise<void> {
  const row: StoredQualificationFailureTrace = {
    id: trace.failure_trace_id,
    trace,
    storedAt: Date.now(),
  };
  const equivalent = (
    left: StoredQualificationFailureTrace,
    right: StoredQualificationFailureTrace,
  ): boolean => sameValue(left.trace, right.trace);
  if (typeof indexedDB === 'undefined') {
    addVolatileImmutable(volatileQualificationFailureTraces, row, equivalent);
    return;
  }
  try {
    await addIndexedImmutable(QUALIFICATION_FAILURE_TRACES, row, equivalent);
  } catch (cause) {
    addVolatileImmutable(volatileQualificationFailureTraces, row, equivalent);
    throw cause;
  }
}

export async function qualificationFailureTrace(
  jobId: string,
): Promise<StoredQualificationFailureTrace | null> {
  const volatile = [...volatileQualificationFailureTraces.values()]
    .find((row) => row.trace.definition.job_id === jobId) ?? null;
  if (typeof indexedDB === 'undefined') return volatile;
  const database = await openDatabase();
  try {
    const rows = await requestResult(
      database.transaction(QUALIFICATION_FAILURE_TRACES, 'readonly')
        .objectStore(QUALIFICATION_FAILURE_TRACES)
        .index('jobId')
        .getAll(jobId),
    ) as StoredQualificationFailureTrace[];
    if (volatile) rows.push(volatile);
    const unique = new Map(rows.map((row) => [row.id, row]));
    if (unique.size > 1) throw immutableConflict(jobId);
    return [...unique.values()][0] ?? null;
  } finally {
    database.close();
  }
}

/** Writes the authority record first and publishes its completeness marker last. */
export async function storeQualificationResultGroup(
  group: QualificationResultGroup,
): Promise<void> {
  const storedAt = Date.now();
  const resultRow: StoredQualificationResult = {
    id: group.result.result_id,
    result: group.result,
    storedAt,
  };
  const markerRow: StoredQualificationCompleteMarker = {
    id: group.complete_marker.marker_id,
    marker: group.complete_marker,
    storedAt,
  };
  const sameResult = (left: StoredQualificationResult, right: StoredQualificationResult): boolean => (
    sameValue(left.result, right.result)
  );
  const sameMarker = (
    left: StoredQualificationCompleteMarker,
    right: StoredQualificationCompleteMarker,
  ): boolean => sameValue(left.marker, right.marker);
  if (typeof indexedDB === 'undefined') {
    addVolatileImmutable(volatileQualificationResults, resultRow, sameResult);
    addVolatileImmutable(volatileQualificationResultMarkers, markerRow, sameMarker);
    return;
  }
  try {
    await addIndexedImmutable(QUALIFICATION_RESULTS, resultRow, sameResult);
  } catch (cause) {
    addVolatileImmutable(volatileQualificationResults, resultRow, sameResult);
    throw cause;
  }
  await addIndexedImmutable(QUALIFICATION_RESULT_MARKERS, markerRow, sameMarker);
}

/** Returns only marker-complete groups; partial authority rows stay hidden and resumable. */
export async function qualificationResultGroup(
  jobId: string,
): Promise<StoredQualificationResultGroup | null> {
  const volatileResult = [...volatileQualificationResults.values()]
    .find((row) => row.result.definition.job_id === jobId) ?? null;
  let result = volatileResult;
  let database: IDBDatabase | null = null;
  if (typeof indexedDB !== 'undefined') {
    database = await openDatabase();
    const indexed = await requestResult(
      database.transaction(QUALIFICATION_RESULTS, 'readonly')
        .objectStore(QUALIFICATION_RESULTS)
        .index('jobId')
        .get(jobId),
    ) as StoredQualificationResult | undefined;
    if (indexed && result && !sameValue(indexed.result, result.result)) {
      database.close();
      throw immutableConflict(jobId);
    }
    result = indexed ?? result;
  }
  if (!result) {
    database?.close();
    return null;
  }
  const volatileMarker = [...volatileQualificationResultMarkers.values()]
    .find((row) => row.marker.definition.result_id === result?.id) ?? null;
  let completeMarker = volatileMarker;
  if (database) {
    const indexed = await requestResult(
      database.transaction(QUALIFICATION_RESULT_MARKERS, 'readonly')
        .objectStore(QUALIFICATION_RESULT_MARKERS)
        .index('resultId')
        .get(result.id),
    ) as StoredQualificationCompleteMarker | undefined;
    database.close();
    if (indexed && completeMarker && !sameValue(indexed.marker, completeMarker.marker)) {
      throw immutableConflict(result.id);
    }
    completeMarker = indexed ?? completeMarker;
  }
  if (!completeMarker || completeMarker.marker.definition.result_id !== result.id) return null;
  return { completeMarker, result };
}

export async function storeQualificationUnlockReceipt(
  receipt: QualificationUnlockReceipt,
): Promise<void> {
  const row: StoredQualificationUnlockReceipt = {
    id: receipt.receipt_id,
    receipt,
    storedAt: Date.now(),
  };
  const equivalent = (
    left: StoredQualificationUnlockReceipt,
    right: StoredQualificationUnlockReceipt,
  ): boolean => sameValue(left.receipt, right.receipt);
  if (typeof indexedDB === 'undefined') {
    addVolatileImmutable(volatileQualificationUnlockReceipts, row, equivalent);
    return;
  }
  try {
    await addIndexedImmutable(QUALIFICATION_UNLOCK_RECEIPTS, row, equivalent);
  } catch (cause) {
    addVolatileImmutable(volatileQualificationUnlockReceipts, row, equivalent);
    throw cause;
  }
}

/** Rebuilds the disposable availability input from receipts with every source child present. */
export async function qualificationUnlockReceipts(): Promise<QualificationUnlockReceipt[]> {
  if (typeof indexedDB === 'undefined') {
    const results = volatileQualificationResults;
    const markers = new Set(
      [...volatileQualificationResultMarkers.values()]
        .map((row) => row.marker.definition.result_id),
    );
    return [...volatileQualificationUnlockReceipts.values()]
      .filter((row) => {
        const result = results.get(row.receipt.definition.result_id)?.result;
        return result?.definition.outcome === 'passed' && markers.has(result.result_id);
      })
      .map((row) => row.receipt);
  }
  const database = await openDatabase();
  try {
    const storeNames = [
      QUALIFICATION_UNLOCK_RECEIPTS,
      QUALIFICATION_RESULTS,
      QUALIFICATION_RESULT_MARKERS,
      QUALIFICATION_REQUESTS,
      QUALIFICATION_JOBS,
      QUALIFICATION_TRIALS,
      QUALIFICATION_CRITERION_DECISIONS,
      QUALIFICATION_FUNCTION_DECISIONS,
      QUALIFICATION_GRADES,
      QUALIFICATION_FAILURE_TRACES,
    ];
    const transaction = database.transaction(storeNames, 'readonly');
    const [receiptRows, resultRows, markerRows, requestRows, jobRows, trialRows,
      criterionRows, functionRows, gradeRows, traceRows] = await Promise.all([
      requestResult(transaction.objectStore(QUALIFICATION_UNLOCK_RECEIPTS).getAll()),
      requestResult(transaction.objectStore(QUALIFICATION_RESULTS).getAll()),
      requestResult(transaction.objectStore(QUALIFICATION_RESULT_MARKERS).getAll()),
      requestResult(transaction.objectStore(QUALIFICATION_REQUESTS).getAll()),
      requestResult(transaction.objectStore(QUALIFICATION_JOBS).getAll()),
      requestResult(transaction.objectStore(QUALIFICATION_TRIALS).getAll()),
      requestResult(transaction.objectStore(QUALIFICATION_CRITERION_DECISIONS).getAll()),
      requestResult(transaction.objectStore(QUALIFICATION_FUNCTION_DECISIONS).getAll()),
      requestResult(transaction.objectStore(QUALIFICATION_GRADES).getAll()),
      requestResult(transaction.objectStore(QUALIFICATION_FAILURE_TRACES).getAll()),
    ]) as [
      StoredQualificationUnlockReceipt[],
      StoredQualificationResult[],
      StoredQualificationCompleteMarker[],
      StoredQualificationRequest[],
      StoredQualificationJob[],
      StoredQualificationTrial[],
      StoredQualificationCriterionDecision[],
      StoredQualificationFunctionDecision[],
      StoredQualificationGrade[],
      StoredQualificationFailureTrace[],
    ];
    for (const row of volatileQualificationUnlockReceipts.values()) receiptRows.push(row);
    for (const row of volatileQualificationResults.values()) resultRows.push(row);
    for (const row of volatileQualificationResultMarkers.values()) markerRows.push(row);
    for (const row of volatileQualificationRequests.values()) requestRows.push(row);
    for (const row of volatileQualificationJobs.values()) jobRows.push(row);
    for (const row of volatileQualificationTrials.values()) trialRows.push(row);
    for (const row of volatileQualificationCriterionDecisions.values()) criterionRows.push(row);
    for (const row of volatileQualificationFunctionDecisions.values()) functionRows.push(row);
    for (const row of volatileQualificationGrades.values()) gradeRows.push(row);
    for (const row of volatileQualificationFailureTraces.values()) traceRows.push(row);
    const receipts = new Map(receiptRows.map((row) => [row.id, row]));
    const results = new Map(resultRows.map((row) => [row.id, row.result]));
    const markers = new Map(markerRows.map((row) => [row.marker.definition.result_id, row.marker]));
    const requestIds = new Set(requestRows.map((row) => row.id));
    const jobIds = new Set(jobRows.map((row) => row.id));
    const trialIds = new Set(trialRows.map((row) => row.id));
    const criterionIds = new Set(criterionRows.map((row) => row.id));
    const functionIds = new Set(functionRows.map((row) => row.id));
    const gradeIds = new Set(gradeRows.map((row) => row.id));
    const traceIds = new Set(traceRows.map((row) => row.id));
    return [...receipts.values()]
      .filter((row) => {
        const result = results.get(row.receipt.definition.result_id);
        if (!result || result.definition.outcome !== 'passed') return false;
        const definition = result.definition;
        const marker = markers.get(result.result_id);
        const expectedChildCount = definition.artifact_ids.length
          + definition.criterion_decision_ids.length
          + definition.grade_ids.length
          + 3
          + Number(definition.failure_trace_id !== null);
        return marker?.definition.child_count === expectedChildCount
          && row.receipt.definition.contract_id === definition.contract_id
          && row.receipt.definition.content_hash === definition.content_hash
          && requestIds.has(definition.request_id)
          && jobIds.has(definition.job_id)
          && definition.artifact_ids.every((id) => trialIds.has(id))
          && definition.criterion_decision_ids.every((id) => criterionIds.has(id))
          && functionIds.has(definition.function_decision_id)
          && definition.grade_ids.every((id) => gradeIds.has(id))
          && (definition.failure_trace_id === null || traceIds.has(definition.failure_trace_id));
      })
      .map((row) => row.receipt)
      .sort((left, right) => left.definition.contract_id.localeCompare(right.definition.contract_id));
  } finally {
    database.close();
  }
}

async function completeQualificationResultById(
  resultId: string,
): Promise<StoredQualificationResultGroup | null> {
  const volatile = volatileQualificationResults.get(resultId);
  const indexed = typeof indexedDB === 'undefined'
    ? undefined
    : await indexedRow<StoredQualificationResult>(QUALIFICATION_RESULTS, resultId);
  if (volatile && indexed && !sameValue(volatile.result, indexed.result)) {
    throw immutableConflict(resultId);
  }
  const result = indexed ?? volatile;
  if (!result) return null;
  const group = await qualificationResultGroup(result.result.definition.job_id);
  return group?.result.id === resultId ? group : null;
}

async function qualificationRequestById(
  requestId: string,
): Promise<StoredQualificationRequest | null> {
  const volatile = volatileQualificationRequests.get(requestId);
  const indexed = typeof indexedDB === 'undefined'
    ? undefined
    : await indexedRow<StoredQualificationRequest>(QUALIFICATION_REQUESTS, requestId);
  if (volatile && indexed && !sameValue(volatile.record, indexed.record)) {
    throw immutableConflict(requestId);
  }
  return indexed ?? volatile ?? null;
}

async function requireV2CaptureAuthority(capture: EngineeringMemoryCapture): Promise<void> {
  if (capture.version !== 2) return;
  const blueprint = capture.blueprint.definition;
  const assembly = capture.assembly_record.definition;
  const generator = capture.generator_record.definition;
  if (blueprint.version !== 2 || assembly.version !== 2 || generator.version !== 2) {
    throw new Error('engineering_record_version_mismatch');
  }
  if (
    blueprint.assembly_record_id !== capture.assembly_record.assembly_record_id
    || blueprint.generator_record_id !== capture.generator_record.generator_record_id
    || assembly.compatibility.generator_record_id !== capture.generator_record.generator_record_id
    || assembly.compatibility.contract_id !== blueprint.contract_id
    || assembly.compatibility.content_hash !== blueprint.content_hash
    || generator.contract_id !== blueprint.contract_id
    || generator.content_hash !== blueprint.content_hash
    || assembly.source.attempt_id !== blueprint.source_attempt_id
    || assembly.source.branch_id !== blueprint.source_branch_id
    || generator.source.attempt_id !== blueprint.source_attempt_id
    || generator.source.branch_id !== blueprint.source_branch_id
    || assembly.source.authority !== generator.source.authority
    || assembly.source.result_id !== generator.source.result_id
  ) {
    throw new Error('engineering_record_authority_mismatch');
  }
  const resultLinks = blueprint.evidence_links.filter((link) => (
    link.evidence_kind === 'qualification_result' && link.availability === 'available'
  ));
  if (blueprint.creation_reason === 'design_capture') {
    if (
      resultLinks.length !== 0
      || generator.source.authority !== 'committed_design'
      || generator.source.result_id !== null
    ) {
      throw new Error('engineering_design_capture_source');
    }
    return;
  }
  if (
    blueprint.creation_reason !== 'result_capture'
    || generator.source.authority !== 'qualification_result'
    || generator.source.result_id === null
    || resultLinks.length !== 1
    || resultLinks[0]?.evidence_id !== generator.source.result_id
  ) {
    throw new Error('engineering_result_capture_source');
  }
  const resultGroup = await completeQualificationResultById(generator.source.result_id);
  if (!resultGroup) throw new Error('engineering_result_unavailable');
  const result = resultGroup.result.result.definition;
  const request = await qualificationRequestById(result.request_id);
  if (
    !request
    || result.contract_id !== blueprint.contract_id
    || result.content_hash !== blueprint.content_hash
    || result.generator_spec_hash !== generator.generator_spec_hash
    || result.assembly_template_hash !== assembly.assembly_template_hash
    || request.record.input.attempt_id !== blueprint.source_attempt_id
    || request.record.input.branch_id !== blueprint.source_branch_id
  ) {
    throw new Error('engineering_result_authority_mismatch');
  }
}

/** Stores canonical assembly and generator children before their blueprint authority. */
export async function storeEngineeringMemoryCapture(
  capture: EngineeringMemoryCapture,
  name: string,
  thumbnail: EngineeringBlueprintThumbnail | null = null,
): Promise<void> {
  await requireV2CaptureAuthority(capture);
  if (
    thumbnail
    && (
      thumbnail.assemblyHash !== capture.assembly_record.definition.assembly_template_hash
      || thumbnail.generatorHash !== capture.generator_record.definition.generator_spec_hash
      || !thumbnail.dataUrl.startsWith('data:image/')
      || thumbnail.width <= 0
      || thumbnail.height <= 0
    )
  ) throw new Error('engineering_thumbnail_authority_mismatch');
  const storedAt = Date.now();
  const assembly: StoredEngineeringAssembly = {
    id: capture.assembly_record.assembly_record_id,
    record: capture.assembly_record,
    storedAt,
  };
  const generator: StoredEngineeringGenerator = {
    id: capture.generator_record.generator_record_id,
    record: capture.generator_record,
    storedAt,
  };
  const blueprint: StoredEngineeringBlueprint = {
    id: capture.blueprint.blueprint_id,
    record: capture.blueprint,
    storedAt,
  };
  const metadata: EngineeringBlueprintMetadata = {
    id: capture.blueprint.blueprint_id,
    blueprintId: capture.blueprint.blueprint_id,
    name: name.trim() || capture.blueprint.definition.contract_id,
    tags: [],
    thumbnail,
    updatedAt: storedAt,
  };
  const sameAssembly = (left: StoredEngineeringAssembly, right: StoredEngineeringAssembly) => (
    sameValue(left.record, right.record)
  );
  const sameGenerator = (left: StoredEngineeringGenerator, right: StoredEngineeringGenerator) => (
    sameValue(left.record, right.record)
  );
  const sameBlueprint = (left: StoredEngineeringBlueprint, right: StoredEngineeringBlueprint) => (
    sameValue(left.record, right.record)
  );
  if (typeof indexedDB === 'undefined') {
    addVolatileImmutable(volatileEngineeringAssemblies, assembly, sameAssembly);
    addVolatileImmutable(volatileEngineeringGenerators, generator, sameGenerator);
    addVolatileImmutable(volatileEngineeringBlueprints, blueprint, sameBlueprint);
    volatileEngineeringBlueprintMetadata.set(metadata.id, metadata);
    return;
  }
  await addIndexedImmutable(ENGINEERING_ASSEMBLIES, assembly, sameAssembly);
  await addIndexedImmutable(ENGINEERING_GENERATORS, generator, sameGenerator);
  await addIndexedImmutable(ENGINEERING_BLUEPRINTS, blueprint, sameBlueprint);
  const database = await openDatabase();
  try {
    await requestResult(
      database.transaction(ENGINEERING_BLUEPRINT_METADATA, 'readwrite')
        .objectStore(ENGINEERING_BLUEPRINT_METADATA)
        .put(metadata),
    );
  } catch (cause) {
    volatileEngineeringBlueprintMetadata.set(metadata.id, metadata);
    throw cause;
  } finally {
    database.close();
  }
}

export async function engineeringBlueprints(): Promise<EngineeringBlueprintEntry[]> {
  if (typeof indexedDB === 'undefined') {
    return [...volatileEngineeringBlueprints.values()]
      .map((row) => {
        const assembly = volatileEngineeringAssemblies
          .get(row.record.definition.assembly_record_id)?.record ?? null;
        const generator = volatileEngineeringGenerators
          .get(row.record.definition.generator_record_id)?.record ?? null;
        return {
          assembly,
          generator,
          metadata: normalizedEngineeringMetadata(
            volatileEngineeringBlueprintMetadata.get(row.id) ?? {
              id: row.id,
              blueprintId: row.id,
              name: row.record.definition.contract_id,
              tags: [],
              thumbnail: null,
              updatedAt: row.storedAt,
            },
          ),
          record: row.record,
          unavailableRelationships: [
            ...(!assembly ? ['assembly' as const] : []),
            ...(!generator ? ['generator' as const] : []),
          ],
        };
      })
      .sort((left, right) => right.metadata.updatedAt - left.metadata.updatedAt);
  }
  const database = await openDatabase();
  try {
    const transaction = database.transaction(
      [
        ENGINEERING_ASSEMBLIES,
        ENGINEERING_GENERATORS,
        ENGINEERING_BLUEPRINTS,
        ENGINEERING_BLUEPRINT_METADATA,
      ],
      'readonly',
    );
    const [assemblies, generators, records, metadata] = await Promise.all([
      requestResult(transaction.objectStore(ENGINEERING_ASSEMBLIES).getAll()),
      requestResult(transaction.objectStore(ENGINEERING_GENERATORS).getAll()),
      requestResult(transaction.objectStore(ENGINEERING_BLUEPRINTS).getAll()),
      requestResult(transaction.objectStore(ENGINEERING_BLUEPRINT_METADATA).getAll()),
    ]) as [
      StoredEngineeringAssembly[],
      StoredEngineeringGenerator[],
      StoredEngineeringBlueprint[],
      EngineeringBlueprintMetadata[],
    ];
    for (const row of volatileEngineeringAssemblies.values()) assemblies.push(row);
    for (const row of volatileEngineeringGenerators.values()) generators.push(row);
    for (const row of volatileEngineeringBlueprints.values()) records.push(row);
    for (const row of volatileEngineeringBlueprintMetadata.values()) metadata.push(row);
    const immutableRows = <T extends { id: string; record: unknown }>(rows: T[]): Map<string, T> => {
      const found = new Map<string, T>();
      for (const row of rows) {
        const existing = found.get(row.id);
        if (existing && !sameValue(existing.record, row.record)) throw immutableConflict(row.id);
        found.set(row.id, row);
      }
      return found;
    };
    const uniqueRecords = immutableRows(records);
    const assembliesById = new Map(
      [...immutableRows(assemblies).values()].map((row) => [row.id, row.record]),
    );
    const generatorsById = new Map(
      [...immutableRows(generators).values()].map((row) => [row.id, row.record]),
    );
    const metadataById = new Map(metadata.map((row) => [row.blueprintId, row]));
    return [...uniqueRecords.values()]
      .map((row) => {
        const assembly = assembliesById.get(row.record.definition.assembly_record_id) ?? null;
        const generator = generatorsById.get(row.record.definition.generator_record_id) ?? null;
        return {
          assembly,
          generator,
          metadata: normalizedEngineeringMetadata(metadataById.get(row.id) ?? {
            id: row.id,
            blueprintId: row.id,
            name: row.record.definition.contract_id,
            tags: [],
            thumbnail: null,
            updatedAt: row.storedAt,
          }),
          record: row.record,
          unavailableRelationships: [
            ...(!assembly ? ['assembly' as const] : []),
            ...(!generator ? ['generator' as const] : []),
          ],
        };
      })
      .sort((left, right) => right.metadata.updatedAt - left.metadata.updatedAt);
  } finally {
    database.close();
  }
}

/**
 * Resolves Revert Generator choices from immutable ancestor authority.
 *
 * A source remains visible when one of its required relationships is missing,
 * corrupt, or unsupported. Only an entry marked `available` may cross into the
 * Rust transition preview, and that entry always carries an exact V2 generator
 * record rather than bytes reconstructed by the shell.
 */
export async function engineeringGeneratorSources(
  context: EngineeringGeneratorSourceContext,
): Promise<EngineeringGeneratorSourceEntry[]> {
  let branchRows = [...volatileAttemptBranches.values()];
  let generatorRows = [...volatileEngineeringGenerators.values()];
  let blueprintRows = [...volatileEngineeringBlueprints.values()];
  let metadataRows = [...volatileEngineeringBlueprintMetadata.values()];
  let requestRows = [...volatileQualificationRequests.values()];
  let resultRows = [...volatileQualificationResults.values()];
  let markerRows = [...volatileQualificationResultMarkers.values()];

  if (typeof indexedDB !== 'undefined') {
    const database = await openDatabase();
    try {
      const transaction = database.transaction(
        [
          ATTEMPT_BRANCHES,
          ENGINEERING_GENERATORS,
          ENGINEERING_BLUEPRINTS,
          ENGINEERING_BLUEPRINT_METADATA,
          QUALIFICATION_REQUESTS,
          QUALIFICATION_RESULTS,
          QUALIFICATION_RESULT_MARKERS,
        ],
        'readonly',
      );
      const indexed = await Promise.all([
        requestResult(transaction.objectStore(ATTEMPT_BRANCHES).getAll()),
        requestResult(transaction.objectStore(ENGINEERING_GENERATORS).getAll()),
        requestResult(transaction.objectStore(ENGINEERING_BLUEPRINTS).getAll()),
        requestResult(transaction.objectStore(ENGINEERING_BLUEPRINT_METADATA).getAll()),
        requestResult(transaction.objectStore(QUALIFICATION_REQUESTS).getAll()),
        requestResult(transaction.objectStore(QUALIFICATION_RESULTS).getAll()),
        requestResult(transaction.objectStore(QUALIFICATION_RESULT_MARKERS).getAll()),
      ]) as [
        StoredAttemptBranchRecord[],
        StoredEngineeringGenerator[],
        StoredEngineeringBlueprint[],
        EngineeringBlueprintMetadata[],
        StoredQualificationRequest[],
        StoredQualificationResult[],
        StoredQualificationCompleteMarker[],
      ];
      branchRows = [...indexed[0], ...branchRows];
      generatorRows = [...indexed[1], ...generatorRows];
      blueprintRows = [...indexed[2], ...blueprintRows];
      metadataRows = [...indexed[3], ...metadataRows];
      requestRows = [...indexed[4], ...requestRows];
      resultRows = [...indexed[5], ...resultRows];
      markerRows = [...indexed[6], ...markerRows];
    } finally {
      database.close();
    }
  }

  const immutableRows = <T extends { id: string }>(
    rows: readonly T[],
    authority: (row: T) => unknown,
  ): { conflicts: Set<string>; rows: Map<string, T> } => {
    const unique = new Map<string, T>();
    const conflicts = new Set<string>();
    for (const row of rows) {
      const prior = unique.get(row.id);
      if (prior && !sameValue(authority(prior), authority(row))) conflicts.add(row.id);
      else unique.set(row.id, row);
    }
    return { conflicts, rows: unique };
  };

  const branches = immutableRows(branchRows, (row) => row.record);
  const generators = immutableRows(generatorRows, (row) => row.record);
  const blueprints = immutableRows(blueprintRows, (row) => row.record);
  const requests = immutableRows(requestRows, (row) => row.record);
  const results = immutableRows(resultRows, (row) => row.result);
  const markers = immutableRows(markerRows, (row) => row.marker);
  const heldCurrent = branches.rows.get(context.currentBranch.branch_id);
  if (heldCurrent && !sameValue(heldCurrent.record, context.currentBranch)) {
    branches.conflicts.add(context.currentBranch.branch_id);
  } else if (!heldCurrent) {
    branches.rows.set(context.currentBranch.branch_id, {
      id: context.currentBranch.branch_id,
      record: context.currentBranch,
      storedAt: 0,
    });
  }

  const metadataByBlueprint = new Map<string, EngineeringBlueprintMetadata>();
  for (const metadata of metadataRows) {
    const prior = metadataByBlueprint.get(metadata.blueprintId);
    if (!prior || prior.updatedAt <= metadata.updatedAt) {
      metadataByBlueprint.set(metadata.blueprintId, normalizedEngineeringMetadata(metadata));
    }
  }
  const markersByResult = new Map<string, StoredQualificationCompleteMarker[]>();
  for (const marker of markers.rows.values()) {
    const resultId = marker.marker.definition.result_id;
    const held = markersByResult.get(resultId);
    if (held) held.push(marker);
    else markersByResult.set(resultId, [marker]);
  }

  const ancestorDistance = new Map<string, number>();
  const ancestorBranches = new Map<string, CanonicalAttemptBranchRecord>();
  const lineageFailures: EngineeringGeneratorSourceEntry[] = [];
  const visited = new Set<string>([context.branchId]);
  let nextBranchId = context.currentBranch.parent_branch_id;
  let distance = 1;
  while (nextBranchId) {
    if (visited.has(nextBranchId)) {
      lineageFailures.push({
        ancestorDistance: distance,
        attemptId: context.attemptId,
        availability: 'corrupt',
        branchId: nextBranchId,
        branchOperation: null,
        contentHash: context.contentHash,
        contractId: context.contractId,
        generator: null,
        generatorHash: '',
        generatorRecordId: null,
        generatorSchema: null,
        id: `branch:${nextBranchId}:lineage_cycle`,
        kind: 'branch',
        name: null,
        reason: 'lineage_cycle',
        resultOutcome: null,
        sourceId: nextBranchId,
        sourceSchema: 0,
      });
      break;
    }
    visited.add(nextBranchId);
    const row = branches.rows.get(nextBranchId);
    if (!row) {
      lineageFailures.push({
        ancestorDistance: distance,
        attemptId: context.attemptId,
        availability: 'unavailable',
        branchId: nextBranchId,
        branchOperation: null,
        contentHash: context.contentHash,
        contractId: context.contractId,
        generator: null,
        generatorHash: '',
        generatorRecordId: null,
        generatorSchema: null,
        id: `branch:${nextBranchId}:missing`,
        kind: 'branch',
        name: null,
        reason: 'missing_ancestor_branch',
        resultOutcome: null,
        sourceId: nextBranchId,
        sourceSchema: 0,
      });
      break;
    }
    if (branches.conflicts.has(nextBranchId)) {
      lineageFailures.push({
        ancestorDistance: distance,
        attemptId: row.record.attempt_id,
        availability: 'corrupt',
        branchId: row.record.branch_id,
        branchOperation: row.record.operation,
        contentHash: context.contentHash,
        contractId: context.contractId,
        generator: null,
        generatorHash: row.record.generator_hash,
        generatorRecordId: null,
        generatorSchema: null,
        id: `branch:${row.record.branch_id}:immutable_conflict`,
        kind: 'branch',
        name: null,
        reason: 'immutable_conflict',
        resultOutcome: null,
        sourceId: row.record.branch_id,
        sourceSchema: row.record.version,
      });
      break;
    }
    if (row.record.attempt_id !== context.attemptId) {
      lineageFailures.push({
        ancestorDistance: distance,
        attemptId: row.record.attempt_id,
        availability: 'corrupt',
        branchId: row.record.branch_id,
        branchOperation: row.record.operation,
        contentHash: context.contentHash,
        contractId: context.contractId,
        generator: null,
        generatorHash: row.record.generator_hash,
        generatorRecordId: null,
        generatorSchema: null,
        id: `branch:${row.record.branch_id}:attempt_mismatch`,
        kind: 'branch',
        name: null,
        reason: 'attempt_mismatch',
        resultOutcome: null,
        sourceId: row.record.branch_id,
        sourceSchema: row.record.version,
      });
      break;
    }
    ancestorDistance.set(row.record.branch_id, distance);
    ancestorBranches.set(row.record.branch_id, row.record);
    nextBranchId = row.record.parent_branch_id;
    distance += 1;
  }

  type SourceSeed = {
    attemptId: string;
    branchId: string;
    branchOperation: CanonicalAttemptBranchRecord['operation'] | null;
    contentHash: string;
    contractId: string;
    directGeneratorRecordId: string | null;
    generatorHash: string;
    kind: EngineeringGeneratorSourceKind;
    name: string | null;
    precondition: EngineeringGeneratorSourceReason | null;
    resultId: string | null;
    resultOutcome: 'passed' | 'failed' | null;
    sourceConflict: boolean;
    sourceId: string;
    sourceSchema: number;
  };

  const resolve = (seed: SourceSeed): EngineeringGeneratorSourceEntry => {
    const sourceDistance = ancestorDistance.get(seed.branchId) ?? Number.MAX_SAFE_INTEGER;
    const unavailable = (
      availability: EngineeringGeneratorSourceAvailability,
      reason: EngineeringGeneratorSourceReason,
      record: EngineeringGeneratorRecord | null = null,
    ): EngineeringGeneratorSourceEntry => ({
      ancestorDistance: sourceDistance,
      attemptId: seed.attemptId,
      availability,
      branchId: seed.branchId,
      branchOperation: seed.branchOperation,
      contentHash: seed.contentHash,
      contractId: seed.contractId,
      generator: availability === 'available' ? record : null,
      generatorHash: seed.generatorHash,
      generatorRecordId: record?.generator_record_id ?? seed.directGeneratorRecordId,
      generatorSchema: record?.definition.version ?? null,
      id: `${seed.kind}:${seed.sourceId}:${(
        record?.generator_record_id ?? seed.directGeneratorRecordId ?? seed.generatorHash
      ) || 'unavailable'}`,
      kind: seed.kind,
      name: seed.name,
      reason,
      resultOutcome: seed.resultOutcome,
      sourceId: seed.sourceId,
      sourceSchema: seed.sourceSchema,
    });

    if (seed.sourceConflict) return unavailable('corrupt', 'immutable_conflict');
    if (seed.precondition) {
      const availability = seed.precondition === 'unsupported_source_schema'
        ? 'unsupported'
        : seed.precondition === 'immutable_conflict'
          ? 'corrupt'
          : 'unavailable';
      return unavailable(availability, seed.precondition);
    }

    let row: StoredEngineeringGenerator | undefined;
    if (seed.directGeneratorRecordId) {
      if (generators.conflicts.has(seed.directGeneratorRecordId)) {
        return unavailable('corrupt', 'immutable_conflict');
      }
      row = generators.rows.get(seed.directGeneratorRecordId);
      if (!row) return unavailable('unavailable', 'missing_generator_record');
    } else {
      const candidates = [...generators.rows.values()].filter((candidate) => {
        const definition = candidate.record.definition;
        if (definition.generator_spec_hash !== seed.generatorHash) return false;
        return definition.version === 1 || (
          definition.source.attempt_id === seed.attemptId
          && definition.source.branch_id === seed.branchId
        );
      });
      row = candidates.find((candidate) => (
        candidate.record.definition.version === 2
        && candidate.record.definition.source.result_id === seed.resultId
      )) ?? candidates.find((candidate) => candidate.record.definition.version === 2)
        ?? candidates[0];
      if (!row) return unavailable('unavailable', 'missing_generator_record');
      if (generators.conflicts.has(row.id)) return unavailable('corrupt', 'immutable_conflict');
    }

    const record = row.record;
    const definition = record.definition;
    if (definition.version !== 2) {
      return unavailable('unsupported', 'unsupported_generator_schema', record);
    }
    if (seed.generatorHash && definition.generator_spec_hash !== seed.generatorHash) {
      return unavailable('corrupt', 'generator_hash_mismatch', record);
    }
    if (
      definition.source.attempt_id !== seed.attemptId
      || definition.source.branch_id !== seed.branchId
    ) {
      return unavailable('corrupt', 'generator_source_mismatch', record);
    }
    if (definition.contract_id !== context.contractId || seed.contractId !== context.contractId) {
      return unavailable('unavailable', 'contract_mismatch', record);
    }
    if (definition.content_hash !== context.contentHash || seed.contentHash !== context.contentHash) {
      return unavailable('unavailable', 'content_mismatch', record);
    }
    return unavailable('available', 'available', record);
  };

  const seeds: SourceSeed[] = [];
  for (const branch of ancestorBranches.values()) {
    seeds.push({
      attemptId: branch.attempt_id,
      branchId: branch.branch_id,
      branchOperation: branch.operation,
      contentHash: context.contentHash,
      contractId: context.contractId,
      directGeneratorRecordId: null,
      generatorHash: branch.generator_hash,
      kind: 'branch',
      name: null,
      precondition: null,
      resultId: null,
      resultOutcome: null,
      sourceConflict: branches.conflicts.has(branch.branch_id),
      sourceId: branch.branch_id,
      sourceSchema: branch.version,
    });
  }

  for (const row of blueprints.rows.values()) {
    const definition = row.record.definition;
    const branchId = definition.version === 1
      ? definition.branch_id
      : definition.source_branch_id;
    const attemptId = definition.source_attempt_id;
    if (attemptId !== context.attemptId || !ancestorBranches.has(branchId)) continue;
    const branch = ancestorBranches.get(branchId) as CanonicalAttemptBranchRecord;
    const directGenerator = generators.rows.get(definition.generator_record_id)?.record ?? null;
    seeds.push({
      attemptId,
      branchId,
      branchOperation: branch.operation,
      contentHash: definition.content_hash,
      contractId: definition.contract_id,
      directGeneratorRecordId: definition.generator_record_id,
      generatorHash: directGenerator?.definition.generator_spec_hash ?? branch.generator_hash,
      kind: 'blueprint',
      name: metadataByBlueprint.get(row.id)?.name ?? null,
      precondition: definition.version === 1 ? 'unsupported_source_schema' : null,
      resultId: definition.version === 2
        ? definition.evidence_links.find((link) => link.evidence_kind === 'qualification_result')?.evidence_id ?? null
        : null,
      resultOutcome: null,
      sourceConflict: blueprints.conflicts.has(row.id),
      sourceId: row.record.blueprint_id,
      sourceSchema: definition.version,
    });
  }

  for (const row of requests.rows.values()) {
    const input = row.record.input;
    if (input.attempt_id !== context.attemptId || !ancestorBranches.has(input.branch_id)) continue;
    const branch = ancestorBranches.get(input.branch_id) as CanonicalAttemptBranchRecord;
    seeds.push({
      attemptId: input.attempt_id,
      branchId: input.branch_id,
      branchOperation: branch.operation,
      contentHash: input.content_hash,
      contractId: input.contract_id,
      directGeneratorRecordId: null,
      generatorHash: input.generator_spec_hash,
      kind: 'qualification_request',
      name: null,
      precondition: null,
      resultId: null,
      resultOutcome: null,
      sourceConflict: requests.conflicts.has(row.id),
      sourceId: row.record.request_id,
      sourceSchema: row.record.version,
    });
  }

  for (const row of results.rows.values()) {
    const definition = row.result.definition;
    const request = requests.rows.get(definition.request_id);
    if (!request) continue;
    const input = request.record.input;
    if (input.attempt_id !== context.attemptId || !ancestorBranches.has(input.branch_id)) continue;
    const branch = ancestorBranches.get(input.branch_id) as CanonicalAttemptBranchRecord;
    const completeMarkers = markersByResult.get(row.result.result_id) ?? [];
    const markerConflict = completeMarkers.length > 1
      || completeMarkers.some((marker) => markers.conflicts.has(marker.id));
    const expectedChildCount = definition.artifact_ids.length
      + definition.criterion_decision_ids.length
      + definition.grade_ids.length
      + 3
      + Number(definition.failure_trace_id !== null);
    const markerComplete = completeMarkers.some((marker) => (
      marker.marker.definition.child_count === expectedChildCount
    ));
    seeds.push({
      attemptId: input.attempt_id,
      branchId: input.branch_id,
      branchOperation: branch.operation,
      contentHash: definition.content_hash,
      contractId: definition.contract_id,
      directGeneratorRecordId: null,
      generatorHash: definition.generator_spec_hash,
      kind: 'qualification_result',
      name: null,
      precondition: markerConflict
        ? 'immutable_conflict'
        : !markerComplete
          ? 'result_incomplete'
          : null,
      resultId: row.result.result_id,
      resultOutcome: definition.outcome,
      sourceConflict: results.conflicts.has(row.id) || requests.conflicts.has(request.id),
      sourceId: row.result.result_id,
      sourceSchema: definition.version,
    });
  }

  const kindOrder: Record<EngineeringGeneratorSourceKind, number> = {
    branch: 0,
    blueprint: 1,
    qualification_request: 2,
    qualification_result: 3,
  };
  return [...seeds.map(resolve), ...lineageFailures].sort((left, right) => (
    left.ancestorDistance - right.ancestorDistance
    || kindOrder[left.kind] - kindOrder[right.kind]
    || left.sourceId.localeCompare(right.sourceId)
  ));
}

async function storeEngineeringAuthority<T extends { id: string; record: unknown }>(
  storeName: string,
  volatile: Map<string, T>,
  row: T,
): Promise<void> {
  const equivalent = (left: T, right: T): boolean => sameValue(left.record, right.record);
  if (typeof indexedDB === 'undefined') {
    addVolatileImmutable(volatile, row, equivalent);
    return;
  }
  try {
    await addIndexedImmutable(storeName, row, equivalent);
  } catch (cause) {
    addVolatileImmutable(volatile, row, equivalent);
    throw cause;
  }
}

export async function storeEngineeringTransition(
  record: EngineeringTransitionReceipt,
): Promise<void> {
  await storeEngineeringAuthority(
    ENGINEERING_TRANSITIONS,
    volatileEngineeringTransitions,
    { id: record.operation_id, record, storedAt: Date.now() },
  );
}

export async function storeEngineeringAssemblyCommit(
  commit: EngineeringAssemblyCommitResult,
): Promise<void> {
  const assemblyDefinition = commit.assembly_record.definition;
  const generatorDefinition = commit.generator_record.definition;
  const receipt = commit.transition_receipt;
  if (
    assemblyDefinition.version !== 2
    || generatorDefinition.version !== 2
    || !commit.attempt_id
    || !commit.branch_id
    || !commit.assembly_template_hash
    || assemblyDefinition.compatibility.generator_record_id
      !== commit.generator_record.generator_record_id
    || assemblyDefinition.compatibility.contract_id !== generatorDefinition.contract_id
    || assemblyDefinition.compatibility.content_hash !== generatorDefinition.content_hash
    || assemblyDefinition.assembly_template_hash !== commit.assembly_template_hash
    || assemblyDefinition.source.authority !== 'committed_design'
    || generatorDefinition.source.authority !== 'committed_design'
    || assemblyDefinition.source.attempt_id !== commit.attempt_id
    || generatorDefinition.source.attempt_id !== commit.attempt_id
    || assemblyDefinition.source.branch_id !== commit.branch_id
    || generatorDefinition.source.branch_id !== commit.branch_id
    || receipt.operation !== 'assembly_commit'
    || receipt.child_attempt_id !== commit.attempt_id
    || receipt.child_branch_id !== commit.branch_id
    || receipt.parent_branch_id !== commit.previous_branch_id
    || receipt.preview_id !== commit.preview_id
    || commit.diff.definition.before_assembly_hash !== commit.previous_assembly_hash
    || commit.diff.definition.after_assembly_hash !== commit.assembly_template_hash
  ) {
    throw new Error('engineering_assembly_commit_authority_mismatch');
  }
  const storedAt = Date.now();
  const assembly: StoredEngineeringAssembly = {
    id: commit.assembly_record.assembly_record_id,
    record: commit.assembly_record,
    storedAt,
  };
  const generator: StoredEngineeringGenerator = {
    id: commit.generator_record.generator_record_id,
    record: commit.generator_record,
    storedAt,
  };
  const sameAssembly = (left: StoredEngineeringAssembly, right: StoredEngineeringAssembly) => (
    sameValue(left.record, right.record)
  );
  const sameGenerator = (left: StoredEngineeringGenerator, right: StoredEngineeringGenerator) => (
    sameValue(left.record, right.record)
  );
  if (typeof indexedDB === 'undefined') {
    addVolatileImmutable(volatileEngineeringAssemblies, assembly, sameAssembly);
    addVolatileImmutable(volatileEngineeringGenerators, generator, sameGenerator);
  } else {
    try {
      await addIndexedImmutable(ENGINEERING_ASSEMBLIES, assembly, sameAssembly);
      await addIndexedImmutable(ENGINEERING_GENERATORS, generator, sameGenerator);
    } catch (cause) {
      addVolatileImmutable(volatileEngineeringAssemblies, assembly, sameAssembly);
      addVolatileImmutable(volatileEngineeringGenerators, generator, sameGenerator);
      throw cause;
    }
  }
  await storeEngineeringDiff(commit.diff);
  await storeEngineeringTransition(commit.transition_receipt);
}

export async function storeEngineeringTransitionCommit(
  commit: EngineeringTransitionCommitResult,
): Promise<void> {
  const assemblyDefinition = commit.assembly_record.definition;
  const generatorDefinition = commit.generator_record.definition;
  const receipt = commit.transition_receipt;
  if (
    (commit.version !== 3 && commit.version !== 4 && commit.version !== 5)
    || receipt.version !== commit.version
    || (receipt.version >= 4 && !Array.isArray(receipt.compatibility_fields))
    || (receipt.version === 3 && receipt.compatibility_fields !== undefined)
    || (receipt.version === 5 && (
      typeof receipt.after_regime_id !== 'string'
      || typeof receipt.after_scenario_hash !== 'string'
      || typeof receipt.before_regime_id !== 'string'
      || typeof receipt.before_scenario_hash !== 'string'
      || receipt.after_regime_id !== commit.regime
      || receipt.after_scenario_hash !== commit.scenario_hash
    ))
    || (receipt.version < 5 && (
      receipt.after_regime_id !== undefined
      || receipt.after_scenario_hash !== undefined
      || receipt.before_regime_id !== undefined
      || receipt.before_scenario_hash !== undefined
    ))
    || assemblyDefinition.version !== 2
    || generatorDefinition.version !== 2
    || !commit.attempt_id
    || !commit.branch_id
    || !commit.assembly_template_hash
    || receipt.operation === undefined
    || receipt.child_attempt_id !== commit.attempt_id
    || receipt.child_branch_id !== commit.branch_id
    || receipt.preview_id !== commit.preview_id
    || receipt.after_assembly_hash !== commit.assembly_template_hash
    || receipt.after_generator_hash !== generatorDefinition.generator_spec_hash
    || assemblyDefinition.assembly_template_hash !== receipt.after_assembly_hash
    || assemblyDefinition.compatibility.generator_record_id
      !== commit.generator_record.generator_record_id
    || assemblyDefinition.compatibility.contract_id !== generatorDefinition.contract_id
    || assemblyDefinition.compatibility.content_hash !== generatorDefinition.content_hash
    || assemblyDefinition.source.authority !== 'committed_design'
    || generatorDefinition.source.authority !== 'committed_design'
    || assemblyDefinition.source.attempt_id !== commit.attempt_id
    || generatorDefinition.source.attempt_id !== commit.attempt_id
    || assemblyDefinition.source.branch_id !== commit.branch_id
    || generatorDefinition.source.branch_id !== commit.branch_id
  ) {
    throw new Error('engineering_transition_commit_authority_mismatch');
  }
  const storedAt = Date.now();
  await storeEngineeringAuthority(
    ENGINEERING_ASSEMBLIES,
    volatileEngineeringAssemblies,
    {
      id: commit.assembly_record.assembly_record_id,
      record: commit.assembly_record,
      storedAt,
    },
  );
  await storeEngineeringAuthority(
    ENGINEERING_GENERATORS,
    volatileEngineeringGenerators,
    {
      id: commit.generator_record.generator_record_id,
      record: commit.generator_record,
      storedAt,
    },
  );
  await storeEngineeringTransition(receipt);
}

function engineeringOperationJournalId(
  previewId: string,
  operation: EngineeringOperationJournal['operation'],
): string {
  return `${operation}:${previewId}`;
}

function normalizedEngineeringOperation(
  row: EngineeringOperationJournal | (Partial<EngineeringOperationJournal> & { id: string }),
): EngineeringOperationJournal {
  return {
    ...row,
    acceptedCommit: row.acceptedCommit ?? null,
    assemblyRecordId: row.assemblyRecordId ?? null,
    childExport: row.childExport ?? null,
    childIdentity: row.childIdentity ?? null,
    childAttemptId: row.childAttemptId ?? null,
    childBranchId: row.childBranchId ?? null,
    error: row.error ?? null,
    expectedEmbodiedHash: row.expectedEmbodiedHash ?? row.priorIdentity?.embodiedHash ?? '',
    operationId: row.operationId ?? null,
    pointerGeneration: row.pointerGeneration ?? null,
    priorClosure: row.priorClosure ?? null,
    priorExport: row.priorExport ?? null,
    priorIdentity: row.priorIdentity ?? null,
    priorSaveId: row.priorSaveId ?? null,
    recoveryState: row.recoveryState ?? null,
    version: 2,
  } as EngineeringOperationJournal;
}

const ENGINEERING_OPERATION_ORDER: Record<EngineeringOperationState, number> = {
  prepared: 0,
  accepted_unpersisted: 1,
  prior_retained: 2,
  child_published: 3,
  pointer_moved: 4,
  complete: 5,
  refused: 5,
  recovery_required: 5,
};

async function putMutableArchiveRow<T extends { id: string }>(
  storeName: string,
  volatile: Map<string, T>,
  row: T,
): Promise<void> {
  volatile.set(row.id, row);
  if (typeof indexedDB === 'undefined') return;
  const database = await openDatabase();
  try {
    const transaction = database.transaction(storeName, 'readwrite');
    const done = transactionDone(transaction);
    await requestResult(transaction.objectStore(storeName).put(row));
    await done;
  } finally {
    database.close();
  }
}

export async function engineeringOperation(
  previewId: string,
): Promise<EngineeringOperationJournal | null> {
  const memoryRow = [...volatileEngineeringOperations.values()]
    .find((row) => row.previewId === previewId);
  const memory = memoryRow ? normalizedEngineeringOperation(memoryRow) : null;
  if (typeof indexedDB === 'undefined') return memory;
  const database = await openDatabase();
  let indexedRowValue: EngineeringOperationJournal | undefined;
  try {
    const transaction = database.transaction(ENGINEERING_OPERATIONS, 'readonly');
    indexedRowValue = await requestResult(
      transaction.objectStore(ENGINEERING_OPERATIONS).index('previewId').get(previewId),
    ) as EngineeringOperationJournal | undefined;
  } finally {
    database.close();
  }
  const indexed = indexedRowValue ? normalizedEngineeringOperation(indexedRowValue) : null;
  if (memory && indexed && memory.updatedAt === indexed.updatedAt && !sameValue(memory, indexed)) {
    throw immutableConflict(memory.id);
  }
  if (!memory) return indexed ?? null;
  if (!indexed) return memory;
  return memory.updatedAt >= indexed.updatedAt ? memory : indexed;
}

export async function engineeringOperations(): Promise<EngineeringOperationJournal[]> {
  const joined = new Map<string, EngineeringOperationJournal>();
  for (const row of volatileEngineeringOperations.values()) {
    joined.set(row.id, normalizedEngineeringOperation(row));
  }
  if (typeof indexedDB !== 'undefined') {
    const database = await openDatabase();
    try {
      const transaction = database.transaction(ENGINEERING_OPERATIONS, 'readonly');
      const rows = await requestResult(
        transaction.objectStore(ENGINEERING_OPERATIONS).getAll(),
      ) as EngineeringOperationJournal[];
      for (const raw of rows) {
        const row = normalizedEngineeringOperation(raw);
        const memory = joined.get(row.id);
        if (memory && memory.updatedAt === row.updatedAt && !sameValue(memory, row)) {
          throw immutableConflict(row.id);
        }
        if (!memory || row.updatedAt > memory.updatedAt) joined.set(row.id, row);
      }
    } finally {
      database.close();
    }
  }
  return [...joined.values()].sort((left, right) => (
    left.updatedAt === right.updatedAt
      ? left.id.localeCompare(right.id)
      : left.updatedAt - right.updatedAt
  ));
}

export async function prepareEngineeringAssemblyOperation(
  preview: EngineeringAssemblyPreview,
  prior: {
    closure: CommissionAttemptRecord;
    exported: RunExported;
    identity: RunIdentity;
  },
): Promise<EngineeringOperationJournal> {
  if (
    !preview.attempt_id
    || !preview.branch_id
    || !preview.attempt_record
    || !preview.assembly_template_hash
    || preview.run_kind !== 'automation_contract'
  ) {
    throw new Error('engineering_operation_preview_authority');
  }
  const id = engineeringOperationJournalId(preview.preview_id, 'assembly_commit');
  const existing = await engineeringOperation(preview.preview_id);
  const authority = {
    expectedAssemblyHash: preview.assembly_template_hash,
    expectedAttemptId: preview.attempt_id,
    expectedBranchId: preview.branch_id,
    expectedContractId: preview.attempt_record.contract_id,
    expectedEmbodiedHash: prior.identity.embodiedHash,
    expectedGeneratorHash: preview.generator_spec_hash,
  };
  if (existing) {
    if (
      existing.expectedAssemblyHash !== authority.expectedAssemblyHash
      || existing.expectedAttemptId !== authority.expectedAttemptId
      || existing.expectedBranchId !== authority.expectedBranchId
      || existing.expectedContractId !== authority.expectedContractId
      || (existing.expectedEmbodiedHash
        && existing.expectedEmbodiedHash !== authority.expectedEmbodiedHash)
      || existing.expectedGeneratorHash !== authority.expectedGeneratorHash
    ) throw immutableConflict(id);
    if (existing.state !== 'prepared') {
      throw new Error('engineering_operation_already_resolved');
    }
    if (
      (existing.priorClosure && !sameValue(existing.priorClosure, prior.closure))
      || (existing.priorExport && !sameValue(existing.priorExport, prior.exported))
      || (existing.priorIdentity && !sameValue(existing.priorIdentity, prior.identity))
    ) throw immutableConflict(id);
    if (
      !existing.expectedEmbodiedHash
      || !existing.priorClosure
      || !existing.priorExport
      || !existing.priorIdentity
    ) {
      const upgraded: EngineeringOperationJournal = {
        ...existing,
        expectedEmbodiedHash: authority.expectedEmbodiedHash,
        priorClosure: prior.closure,
        priorExport: prior.exported,
        priorIdentity: prior.identity,
        updatedAt: Date.now(),
        version: 2,
      };
      await putMutableArchiveRow(
        ENGINEERING_OPERATIONS,
        volatileEngineeringOperations,
        upgraded,
      );
      return upgraded;
    }
    return existing;
  }
  const prepared: EngineeringOperationJournal = {
    acceptedCommit: null,
    id,
    assemblyRecordId: null,
    childExport: null,
    childIdentity: null,
    childAttemptId: null,
    childBranchId: null,
    error: null,
    ...authority,
    operation: 'assembly_commit',
    operationId: null,
    pointerGeneration: null,
    previewId: preview.preview_id,
    priorClosure: prior.closure,
    priorExport: prior.exported,
    priorIdentity: prior.identity,
    priorSaveId: null,
    recoveryState: null,
    state: 'prepared',
    updatedAt: Date.now(),
    version: 2,
  };
  await putMutableArchiveRow(
    ENGINEERING_OPERATIONS,
    volatileEngineeringOperations,
    prepared,
  );
  return prepared;
}

export async function prepareEngineeringTransitionOperation(
  preview: EngineeringRunTransitionPreview,
  prior: {
    closure: CommissionAttemptRecord;
    exported: RunExported;
    identity: RunIdentity;
  },
): Promise<EngineeringOperationJournal> {
  const { guard, operation } = preview.definition;
  const id = engineeringOperationJournalId(preview.preview_id, operation);
  const authority = {
    expectedAssemblyHash: guard.assembly_hash,
    expectedAttemptId: guard.attempt_id,
    expectedBranchId: guard.branch_id,
    expectedContractId: guard.contract_id,
    expectedEmbodiedHash: guard.embodied_hash,
    expectedGeneratorHash: guard.generator_hash,
  };
  const existing = await engineeringOperation(preview.preview_id);
  if (existing) {
    if (
      existing.id !== id
      || existing.operation !== operation
      || existing.expectedAssemblyHash !== authority.expectedAssemblyHash
      || existing.expectedAttemptId !== authority.expectedAttemptId
      || existing.expectedBranchId !== authority.expectedBranchId
      || existing.expectedContractId !== authority.expectedContractId
      || (existing.expectedEmbodiedHash
        && existing.expectedEmbodiedHash !== authority.expectedEmbodiedHash)
      || existing.expectedGeneratorHash !== authority.expectedGeneratorHash
      || (existing.priorClosure && !sameValue(existing.priorClosure, prior.closure))
      || (existing.priorExport && !sameValue(existing.priorExport, prior.exported))
      || (existing.priorIdentity && !sameValue(existing.priorIdentity, prior.identity))
    ) throw immutableConflict(id);
    if (existing.state !== 'prepared') {
      throw new Error('engineering_operation_already_resolved');
    }
    if (
      !existing.expectedEmbodiedHash
      || !existing.priorClosure
      || !existing.priorExport
      || !existing.priorIdentity
    ) {
      const upgraded: EngineeringOperationJournal = {
        ...existing,
        expectedEmbodiedHash: authority.expectedEmbodiedHash,
        priorClosure: prior.closure,
        priorExport: prior.exported,
        priorIdentity: prior.identity,
        updatedAt: Date.now(),
        version: 2,
      };
      await putMutableArchiveRow(
        ENGINEERING_OPERATIONS,
        volatileEngineeringOperations,
        upgraded,
      );
      return upgraded;
    }
    return existing;
  }
  const prepared: EngineeringOperationJournal = {
    acceptedCommit: null,
    id,
    assemblyRecordId: null,
    childExport: null,
    childIdentity: null,
    childAttemptId: null,
    childBranchId: null,
    error: null,
    ...authority,
    operation,
    operationId: null,
    pointerGeneration: null,
    previewId: preview.preview_id,
    priorClosure: prior.closure,
    priorExport: prior.exported,
    priorIdentity: prior.identity,
    priorSaveId: null,
    recoveryState: null,
    state: 'prepared',
    updatedAt: Date.now(),
    version: 2,
  };
  await putMutableArchiveRow(
    ENGINEERING_OPERATIONS,
    volatileEngineeringOperations,
    prepared,
  );
  return prepared;
}

export async function advanceEngineeringOperation(
  previewId: string,
  state: EngineeringOperationState,
  patch: Partial<Pick<
    EngineeringOperationJournal,
    | 'acceptedCommit'
    | 'assemblyRecordId'
    | 'childExport'
    | 'childIdentity'
    | 'childAttemptId'
    | 'childBranchId'
    | 'error'
    | 'operationId'
    | 'pointerGeneration'
    | 'priorSaveId'
    | 'recoveryState'
  >> = {},
): Promise<EngineeringOperationJournal> {
  const current = await engineeringOperation(previewId);
  if (!current) throw new Error('engineering_operation_missing');
  if (
    (current.state === 'complete' || current.state === 'refused')
    && state !== current.state
  ) throw new Error('engineering_operation_terminal');
  if (
    current.state !== 'recovery_required'
    && ENGINEERING_OPERATION_ORDER[state] < ENGINEERING_OPERATION_ORDER[current.state]
    && state !== 'recovery_required'
  ) throw new Error('engineering_operation_state_regression');
  const next: EngineeringOperationJournal = {
    ...current,
    ...patch,
    state,
    updatedAt: Date.now(),
  };
  for (const key of [
    'assemblyRecordId',
    'childAttemptId',
    'childBranchId',
    'operationId',
    'priorSaveId',
  ] as const) {
    if (current[key] && next[key] !== current[key]) throw immutableConflict(current.id);
  }
  for (const key of ['acceptedCommit', 'childExport', 'childIdentity'] as const) {
    if (current[key] && !sameValue(current[key], next[key])) throw immutableConflict(current.id);
  }
  await putMutableArchiveRow(
    ENGINEERING_OPERATIONS,
    volatileEngineeringOperations,
    next,
  );
  return next;
}

function sameAutomationSessionSave(
  left: StoredAutomationSessionSave,
  right: StoredAutomationSessionSave,
): boolean {
  const { storedAt: _leftStoredAt, ...leftAuthority } = left;
  const { storedAt: _rightStoredAt, ...rightAuthority } = right;
  return sameValue(leftAuthority, rightAuthority);
}

async function persistAutomationSessionSave(
  save: StoredAutomationSessionSave,
): Promise<void> {
  if (typeof indexedDB === 'undefined') {
    addVolatileImmutable(
      volatileAutomationSessionSaves,
      save,
      sameAutomationSessionSave,
    );
    return;
  }
  try {
    await addIndexedImmutable(
      AUTOMATION_SESSION_SAVES,
      save,
      sameAutomationSessionSave,
    );
  } catch (cause) {
    addVolatileImmutable(
      volatileAutomationSessionSaves,
      save,
      sameAutomationSessionSave,
    );
    throw cause;
  }
}

export async function storeAutomationSessionSave(
  identity: RunIdentity,
  exported: RunExported,
): Promise<StoredAutomationSessionSave> {
  const attempt = identity.attemptRecord;
  if (
    identity.runKind !== 'automation_contract'
    || !attempt
    || !identity.attemptId
    || !identity.branchId
    || !identity.assemblyHash
    || exported.sha256.length !== 64
    || exported.embodied_state_hash.length !== 64
  ) throw new Error('automation_session_save_authority');
  const save: StoredAutomationSessionSave = {
    id: exported.sha256,
    assemblyHash: identity.assemblyHash,
    attemptId: identity.attemptId,
    branchId: identity.branchId,
    contentHash: attempt.content_hash,
    contractId: attempt.contract_id,
    embodiedHash: exported.embodied_state_hash,
    generatorHash: identity.generatorHash,
    payload: exported.text,
    protocolVersion: PROTOCOL_VERSION,
    runKind: 'automation_contract',
    storedAt: Date.now(),
    version: 1,
  };
  await persistAutomationSessionSave(save);
  return save;
}

export async function automationSessionSave(
  saveId: string,
): Promise<StoredAutomationSessionSave | null> {
  const memory = volatileAutomationSessionSaves.get(saveId);
  if (typeof indexedDB === 'undefined') return memory ?? null;
  const indexed = await indexedRow<StoredAutomationSessionSave>(AUTOMATION_SESSION_SAVES, saveId);
  if (memory && indexed && !sameAutomationSessionSave(memory, indexed)) {
    throw immutableConflict(saveId);
  }
  return indexed ?? memory ?? null;
}

export async function publishEngineeringActiveSession(
  commit: EngineeringAssemblyCommitResult | EngineeringTransitionCommitResult,
  exported: RunExported,
): Promise<ActiveSessionPointer> {
  const attempt = commit.attempt_record;
  const assembly = commit.assembly_record.definition;
  const generator = commit.generator_record.definition;
  if (
    !attempt
    || !commit.attempt_id
    || !commit.branch_id
    || !commit.assembly_template_hash
    || commit.run_kind !== 'automation_contract'
    || assembly.version !== 2
    || generator.version !== 2
    || exported.sha256.length !== 64
    || exported.embodied_state_hash.length !== 64
  ) throw new Error('engineering_active_session_authority');
  const save: StoredAutomationSessionSave = {
    id: exported.sha256,
    assemblyHash: commit.assembly_template_hash,
    attemptId: commit.attempt_id,
    branchId: commit.branch_id,
    contentHash: attempt.content_hash,
    contractId: attempt.contract_id,
    embodiedHash: exported.embodied_state_hash,
    generatorHash: generator.generator_spec_hash,
    payload: exported.text,
    protocolVersion: PROTOCOL_VERSION,
    runKind: 'automation_contract',
    storedAt: Date.now(),
    version: 1,
  };
  await persistAutomationSessionSave(save);
  const memoryPointer = volatileActiveSessionPointers.get(ACTIVE_AUTOMATION_POINTER_ID);
  const indexedPointer = typeof indexedDB === 'undefined'
    ? undefined
    : await indexedRow<ActiveSessionPointer>(ACTIVE_SESSION_POINTERS, ACTIVE_AUTOMATION_POINTER_ID);
  if (
    memoryPointer
    && indexedPointer
    && memoryPointer.pointerGeneration === indexedPointer.pointerGeneration
    && !sameValue(memoryPointer, indexedPointer)
  ) throw immutableConflict(ACTIVE_AUTOMATION_POINTER_ID);
  const currentPointer = !memoryPointer
    ? indexedPointer
    : !indexedPointer || memoryPointer.pointerGeneration >= indexedPointer.pointerGeneration
      ? memoryPointer
      : indexedPointer;
  if (currentPointer?.operationId === commit.transition_receipt.operation_id) {
    if (
      currentPointer.assemblyHash !== save.assemblyHash
      || currentPointer.attemptId !== save.attemptId
      || currentPointer.branchId !== save.branchId
      || currentPointer.contentHash !== save.contentHash
      || currentPointer.contractId !== save.contractId
      || currentPointer.generatorHash !== save.generatorHash
      || currentPointer.saveId !== save.id
    ) throw immutableConflict(ACTIVE_AUTOMATION_POINTER_ID);
    return currentPointer;
  }
  const pointer: ActiveSessionPointer = {
    id: ACTIVE_AUTOMATION_POINTER_ID,
    assemblyHash: save.assemblyHash,
    attemptId: save.attemptId,
    branchId: save.branchId,
    contentHash: save.contentHash,
    contractId: save.contractId,
    generatorHash: save.generatorHash,
    operationId: commit.transition_receipt.operation_id,
    pointerGeneration: (currentPointer?.pointerGeneration ?? 0) + 1,
    protocolVersion: PROTOCOL_VERSION,
    runKind: 'automation_contract',
    saveId: save.id,
    updatedAt: Date.now(),
    version: 1,
  };
  await putMutableArchiveRow(
    ACTIVE_SESSION_POINTERS,
    volatileActiveSessionPointers,
    pointer,
  );
  return pointer;
}

export async function activeSessionPointer(): Promise<ActiveSessionPointer | null> {
  const memory = volatileActiveSessionPointers.get(ACTIVE_AUTOMATION_POINTER_ID);
  if (typeof indexedDB === 'undefined') return memory ?? null;
  const indexed = await indexedRow<ActiveSessionPointer>(
    ACTIVE_SESSION_POINTERS,
    ACTIVE_AUTOMATION_POINTER_ID,
  );
  if (!memory) return indexed ?? null;
  if (!indexed) return memory;
  if (
    memory.pointerGeneration === indexed.pointerGeneration
    && !sameValue(memory, indexed)
  ) throw immutableConflict(ACTIVE_AUTOMATION_POINTER_ID);
  return memory.pointerGeneration >= indexed.pointerGeneration ? memory : indexed;
}

async function manualEngineeringRecovery(
  journal: EngineeringOperationJournal,
  error: string,
): Promise<EngineeringOperationRecovery> {
  const retained = await advanceEngineeringOperation(
    journal.previewId,
    'recovery_required',
    { error },
  ).catch(() => ({ ...journal, error, state: 'recovery_required' as const }));
  return {
    error,
    operation: retained.operation,
    operationId: retained.operationId,
    previewId: retained.previewId,
    state: retained.state,
    status: 'manual_recovery',
  };
}

export async function recoverEngineeringOperation(
  previewId: string,
): Promise<EngineeringOperationRecovery> {
  let journal = await engineeringOperation(previewId);
  if (!journal) throw new Error('engineering_operation_missing');
  if (journal.state === 'complete') {
    return {
      error: journal.error,
      operation: journal.operation,
      operationId: journal.operationId,
      previewId,
      state: journal.state,
      status: 'complete',
    };
  }
  if (journal.state === 'refused') {
    return {
      error: journal.error,
      operation: journal.operation,
      operationId: journal.operationId,
      previewId,
      state: journal.state,
      status: 'refused',
    };
  }
  if (journal.state === 'prepared' && !journal.acceptedCommit) {
    return {
      error: journal.error,
      operation: journal.operation,
      operationId: null,
      previewId,
      state: journal.state,
      status: 'prepared',
    };
  }

  try {
    const pointer = await activeSessionPointer();
    if (
      journal.operationId
      && pointer?.operationId === journal.operationId
      && pointer.branchId === journal.childBranchId
    ) {
      journal = await advanceEngineeringOperation(previewId, 'complete', {
        error: null,
        pointerGeneration: pointer.pointerGeneration,
      });
      return {
        error: null,
        operation: journal.operation,
        operationId: journal.operationId,
        previewId,
        state: journal.state,
        status: 'recovered',
      };
    }
    if (
      journal.state === 'pointer_moved'
      && journal.pointerGeneration !== null
      && pointer
      && pointer.pointerGeneration > journal.pointerGeneration
    ) {
      journal = await advanceEngineeringOperation(previewId, 'complete', { error: null });
      return {
        error: null,
        operation: journal.operation,
        operationId: journal.operationId,
        previewId,
        state: journal.state,
        status: 'recovered',
      };
    }

    if (!journal.priorClosure || !journal.priorExport || !journal.priorIdentity) {
      return manualEngineeringRecovery(journal, 'engineering_recovery_prior_authority_missing');
    }
    const priorSave = await storeAutomationSessionSave(journal.priorIdentity, journal.priorExport);
    await storeRunLineage(journal.priorIdentity);
    await storeCommissionAttempt(journal.priorClosure);
    if (
      journal.state === 'recovery_required'
      || ENGINEERING_OPERATION_ORDER[journal.state]
        < ENGINEERING_OPERATION_ORDER.prior_retained
    ) {
      journal = await advanceEngineeringOperation(previewId, 'prior_retained', {
        error: null,
        priorSaveId: priorSave.id,
      });
    }

    const commit = journal.acceptedCommit;
    const childIdentity = journal.childIdentity;
    if (!commit || !childIdentity) {
      return manualEngineeringRecovery(journal, 'engineering_recovery_child_authority_missing');
    }
    if (commit.transition_receipt.operation === 'assembly_commit') {
      await storeEngineeringAssemblyCommit(commit as EngineeringAssemblyCommitResult);
    } else {
      await storeEngineeringTransitionCommit(commit as EngineeringTransitionCommitResult);
    }
    await storeRunLineage(childIdentity);
    if (
      journal.state === 'recovery_required'
      || ENGINEERING_OPERATION_ORDER[journal.state]
        < ENGINEERING_OPERATION_ORDER.child_published
    ) {
      journal = await advanceEngineeringOperation(previewId, 'child_published', { error: null });
    }

    if (!journal.childExport) {
      return manualEngineeringRecovery(journal, 'engineering_recovery_child_save_missing');
    }
    const moved = await publishEngineeringActiveSession(commit, journal.childExport);
    if (
      journal.state === 'recovery_required'
      || ENGINEERING_OPERATION_ORDER[journal.state]
        < ENGINEERING_OPERATION_ORDER.pointer_moved
    ) {
      journal = await advanceEngineeringOperation(previewId, 'pointer_moved', {
        error: null,
        pointerGeneration: moved.pointerGeneration,
      });
    }
    journal = await advanceEngineeringOperation(previewId, 'complete', {
      error: null,
      recoveryState: 'recovered',
    });
    return {
      error: null,
      operation: journal.operation,
      operationId: journal.operationId,
      previewId,
      state: journal.state,
      status: 'recovered',
    };
  } catch (cause) {
    return manualEngineeringRecovery(
      journal,
      cause instanceof Error ? cause.message : 'engineering_recovery_failed',
    );
  }
}

export async function auditEngineeringOperations(): Promise<EngineeringOperationRecovery[]> {
  const journals = await engineeringOperations();
  const outcomes: EngineeringOperationRecovery[] = [];
  for (const journal of journals) {
    outcomes.push(await recoverEngineeringOperation(journal.previewId));
  }
  return outcomes;
}

/** Retained for callers that still name the original assembly-only journal. */
export async function recoverEngineeringAssemblyOperation(
  previewId: string,
): Promise<EngineeringOperationRecovery> {
  return recoverEngineeringOperation(previewId);
}

/** Retained for callers that still name the original assembly-only audit. */
export async function auditEngineeringAssemblyOperations(): Promise<EngineeringOperationRecovery[]> {
  return auditEngineeringOperations();
}

export async function storeEngineeringDiff(
  record: EngineeringDiffReport | EngineeringAssemblyDiff,
): Promise<void> {
  await storeEngineeringAuthority(
    ENGINEERING_DIFFS,
    volatileEngineeringDiffs,
    { id: record.diff_id, record, storedAt: Date.now() },
  );
}

export async function storeEngineeringCompatibility(
  record: EngineeringCompatibilityReport,
): Promise<void> {
  await storeEngineeringAuthority(
    ENGINEERING_COMPATIBILITY,
    volatileEngineeringCompatibility,
    { id: record.compatibility_id, record, storedAt: Date.now() },
  );
}

export async function storeEngineeringAdaptation(
  record: EngineeringAssemblyAdaptationRecord,
): Promise<void> {
  await storeEngineeringAuthority(
    ENGINEERING_ADAPTATIONS,
    volatileEngineeringAdaptations,
    { id: record.adaptation_id, record, storedAt: Date.now() },
  );
}

export async function storeEngineeringComparative(
  record: ComparativeQualificationRecord,
): Promise<void> {
  await storeEngineeringAuthority(
    ENGINEERING_COMPARATIVES,
    volatileEngineeringComparatives,
    { id: record.comparative_id, record, storedAt: Date.now() },
  );
}

async function engineeringAuthorityRecord<T extends { id: string; record: unknown }>(
  storeName: string,
  volatile: Map<string, T>,
  id: string,
): Promise<T['record'] | null> {
  const memory = volatile.get(id);
  if (typeof indexedDB === 'undefined') return memory?.record ?? null;
  const indexed = await indexedRow<T>(storeName, id);
  if (memory && indexed && !sameValue(memory.record, indexed.record)) {
    throw immutableConflict(id);
  }
  return indexed?.record ?? memory?.record ?? null;
}

export async function engineeringTransition(
  operationId: string,
): Promise<EngineeringTransitionReceipt | null> {
  return engineeringAuthorityRecord(
    ENGINEERING_TRANSITIONS,
    volatileEngineeringTransitions,
    operationId,
  ) as Promise<EngineeringTransitionReceipt | null>;
}

export async function engineeringTransitionByPreview(
  previewId: string,
): Promise<EngineeringTransitionReceipt | null> {
  const memory = [...volatileEngineeringTransitions.values()]
    .find((row) => row.record.preview_id === previewId);
  if (typeof indexedDB === 'undefined') return memory?.record ?? null;
  const database = await openDatabase();
  try {
    const transaction = database.transaction(ENGINEERING_TRANSITIONS, 'readonly');
    const indexed = await requestResult(
      transaction.objectStore(ENGINEERING_TRANSITIONS).index('previewId').get(previewId),
    ) as StoredEngineeringTransition | undefined;
    if (memory && indexed && !sameValue(memory.record, indexed.record)) {
      throw immutableConflict(previewId);
    }
    return indexed?.record ?? memory?.record ?? null;
  } finally {
    database.close();
  }
}

export async function engineeringDiff(
  diffId: string,
): Promise<EngineeringDiffReport | EngineeringAssemblyDiff | null> {
  return engineeringAuthorityRecord(
    ENGINEERING_DIFFS,
    volatileEngineeringDiffs,
    diffId,
  ) as Promise<EngineeringDiffReport | EngineeringAssemblyDiff | null>;
}

export async function engineeringCompatibility(
  compatibilityId: string,
): Promise<EngineeringCompatibilityReport | null> {
  return engineeringAuthorityRecord(
    ENGINEERING_COMPATIBILITY,
    volatileEngineeringCompatibility,
    compatibilityId,
  ) as Promise<EngineeringCompatibilityReport | null>;
}

export async function engineeringAdaptation(
  adaptationId: string,
): Promise<EngineeringAssemblyAdaptationRecord | null> {
  return engineeringAuthorityRecord(
    ENGINEERING_ADAPTATIONS,
    volatileEngineeringAdaptations,
    adaptationId,
  ) as Promise<EngineeringAssemblyAdaptationRecord | null>;
}

export async function engineeringComparative(
  comparativeId: string,
): Promise<ComparativeQualificationRecord | null> {
  return engineeringAuthorityRecord(
    ENGINEERING_COMPARATIVES,
    volatileEngineeringComparatives,
    comparativeId,
  ) as Promise<ComparativeQualificationRecord | null>;
}

export async function engineeringMigrationJournal(): Promise<EngineeringMigrationJournal> {
  if (typeof indexedDB === 'undefined') {
    const existing = volatileEngineeringMigrations.get(ENGINEERING_V12_MIGRATION_ID);
    if (existing && (existing.state === 'complete' || existing.state === 'recovery_required')) {
      return existing;
    }
    const baseline = existing ?? preparedEngineeringMigration(0);
    const created = completedEngineeringMigration(baseline, {
      [ENGINEERING_ASSEMBLIES]: [...volatileEngineeringAssemblies.values()],
      [ENGINEERING_GENERATORS]: [...volatileEngineeringGenerators.values()],
      [ENGINEERING_BLUEPRINTS]: [...volatileEngineeringBlueprints.values()],
    });
    volatileEngineeringMigrations.set(created.id, created);
    return created;
  }
  const database = await openDatabase();
  try {
    const indexed = await readEngineeringMigration(database);
    const volatile = volatileEngineeringMigrations.get(ENGINEERING_V12_MIGRATION_ID);
    if (volatile?.state === 'recovery_required') return volatile;
    return indexed ?? volatile ?? preparedEngineeringMigration(VERSION);
  } finally {
    database.close();
  }
}

export async function commissionAttempts(contractId?: string): Promise<CommissionAttemptRecord[]> {
  if (typeof indexedDB === 'undefined') {
    return [...volatileCommissionAttempts.values()]
      .filter((record) => !contractId || record.contractId === contractId)
      .sort((left, right) => right.recordedAt - left.recordedAt);
  }
  const database = await openDatabase();
  try {
    const transaction = database.transaction(COMMISSION_ATTEMPTS, 'readonly');
    const store = transaction.objectStore(COMMISSION_ATTEMPTS);
    const records = contractId
      ? await requestResult(store.index('contractId').getAll(contractId)) as CommissionAttemptRecord[]
      : await requestResult(store.getAll()) as CommissionAttemptRecord[];
    const merged = new Map(records.map((record) => [record.id, record]));
    for (const record of volatileCommissionAttempts.values()) {
      if (!contractId || record.contractId === contractId) merged.set(record.id, record);
    }
    return [...merged.values()].sort((left, right) => right.recordedAt - left.recordedAt);
  } finally {
    database.close();
  }
}

export async function storeHoldoutSuite(suite: HoldoutSuite): Promise<void> {
  if (typeof indexedDB === 'undefined') {
    volatileHoldouts.set(suite.id, suite);
    return;
  }
  const database = await openDatabase();
  try {
    const transaction = database.transaction(HOLDOUTS, 'readwrite');
    await requestResult(transaction.objectStore(HOLDOUTS).put(suite));
  } finally {
    database.close();
  }
}

export async function holdoutSuites(): Promise<HoldoutSuite[]> {
  if (typeof indexedDB === 'undefined') {
    return [...volatileHoldouts.values()].sort((left, right) => right.createdAt - left.createdAt);
  }
  const database = await openDatabase();
  try {
    const transaction = database.transaction(HOLDOUTS, 'readonly');
    const suites = await requestResult(transaction.objectStore(HOLDOUTS).getAll()) as HoldoutSuite[];
    return suites.sort((left, right) => right.createdAt - left.createdAt);
  } finally {
    database.close();
  }
}

async function digest(value: string): Promise<string> {
  if (typeof crypto !== 'undefined' && crypto.subtle) {
    const bytes = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(value));
    return [...new Uint8Array(bytes)].map((byte) => byte.toString(16).padStart(2, '0')).join('');
  }
  let hash = 2_166_136_261;
  for (let place = 0; place < value.length; place += 1) {
    hash ^= value.charCodeAt(place);
    hash = Math.imul(hash, 16_777_619);
  }
  return (hash >>> 0).toString(16).padStart(8, '0');
}

export async function sealedHoldoutSuite(
  scenario: AnalysisScenario,
  identity: RunIdentity,
): Promise<HoldoutSuite> {
  const now = Date.now();
  const random = new Uint32Array(4);
  if (typeof crypto !== 'undefined' && crypto.getRandomValues) crypto.getRandomValues(random);
  else random.set([now >>> 0, Math.floor(now / 0x1_0000_0000), 0x484f_4c44, 0x4f55_5432]);
  const hiddenSuiteId = [...random].map((value) => value.toString(16).padStart(8, '0')).join('');
  const candidate = JSON.stringify(scenario);
  const sealedBeforeCandidateHash = await digest(candidate);
  const hiddenSuiteVersionHash = await digest(`${hiddenSuiteId}:${sealedBeforeCandidateHash}:v1`);
  return {
    id: `holdout:${scenario.id}:${hiddenSuiteVersionHash.slice(0, 12)}`,
    schemaVersion: 2,
    scenarioId: scenario.id,
    scenarioHash: identity.scenarioHash,
    embodiedStateHash: identity.embodiedHash,
    generatorHash: identity.generatorHash,
    hiddenSuiteId,
    hiddenSuiteVersionHash,
    sealedBeforeCandidateHash,
    suiteSeed: random[0] >>> 0,
    createdAt: now,
    updatedAt: now,
    status: 'sealed',
    trials: 8,
    requiredPasses: 7,
    passed: null,
    contaminationReason: null,
  };
}

function requestResult<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error('archive_request_failed'));
  });
}

function transactionDone(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error('archive_transaction_aborted'));
    transaction.onerror = () => reject(transaction.error ?? new Error('archive_transaction_failed'));
  });
}

function sameValue(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function sameCommissionAttempt(
  left: CommissionAttemptRecord,
  right: CommissionAttemptRecord,
): boolean {
  const { recordedAt: _leftRecordedAt, ...leftCanonical } = left;
  const { recordedAt: _rightRecordedAt, ...rightCanonical } = right;
  return sameValue(leftCanonical, rightCanonical);
}

function immutableConflict(id: string): Error {
  return new Error(`archive_immutable_conflict:${id}`);
}

function addVolatileImmutable<T extends { id: string }>(
  records: Map<string, T>,
  row: T,
  equivalent: (left: T, right: T) => boolean,
): void {
  const existing = records.get(row.id);
  if (existing) {
    if (!equivalent(existing, row)) throw immutableConflict(row.id);
    return;
  }
  records.set(row.id, row);
}

async function indexedRow<T>(storeName: string, id: string): Promise<T | undefined> {
  const database = await openDatabase();
  try {
    const transaction = database.transaction(storeName, 'readonly');
    return await requestResult(transaction.objectStore(storeName).get(id)) as T | undefined;
  } finally {
    database.close();
  }
}

async function addIndexedImmutable<T extends { id: string }>(
  storeName: string,
  row: T,
  equivalent: (left: T, right: T) => boolean,
): Promise<void> {
  const existing = await indexedRow<T>(storeName, row.id);
  if (existing) {
    if (!equivalent(existing, row)) throw immutableConflict(row.id);
    return;
  }
  const database = await openDatabase();
  try {
    const transaction = database.transaction(storeName, 'readwrite');
    try {
      await requestResult(transaction.objectStore(storeName).add(row));
    } catch (cause) {
      if (!(cause instanceof DOMException) || cause.name !== 'ConstraintError') throw cause;
      const raced = await indexedRow<T>(storeName, row.id);
      if (!raced || !equivalent(raced, row)) throw immutableConflict(row.id);
    }
  } finally {
    database.close();
  }
}

export async function storeArchiveRecord(record: ArchiveRecord): Promise<void> {
  if (typeof indexedDB === 'undefined') {
    volatileRecords.set(record.id, record);
    return;
  }
  const database = await openDatabase();
  try {
    const transaction = database.transaction(RUNS, 'readwrite');
    await requestResult(transaction.objectStore(RUNS).put(record));
  } finally {
    database.close();
  }
}

export async function archiveRecords(): Promise<ArchiveRecord[]> {
  if (typeof indexedDB === 'undefined') {
    return [...volatileRecords.values()]
      .map(normalizeArchiveRecord)
      .sort((left, right) => right.createdAt - left.createdAt);
  }
  const database = await openDatabase();
  try {
    const transaction = database.transaction(RUNS, 'readonly');
    const records = await requestResult(transaction.objectStore(RUNS).getAll()) as ArchiveRecord[];
    return records
      .map(normalizeArchiveRecord)
      .sort((left, right) => right.createdAt - left.createdAt);
  } finally {
    database.close();
  }
}

export async function removeArchiveRecord(id: string): Promise<void> {
  if (typeof indexedDB === 'undefined') {
    volatileRecords.delete(id);
    return;
  }
  const database = await openDatabase();
  try {
    const transaction = database.transaction(RUNS, 'readwrite');
    await requestResult(transaction.objectStore(RUNS).delete(id));
  } finally {
    database.close();
  }
}

export function recordFromExport(
  text: string,
  scenario: AnalysisScenario,
  evidence: ArchiveRecord['evidence'],
  embodiedStateHash?: string,
): ArchiveRecord {
  const file = JSON.parse(text) as {
    payload: {
      run_id: string;
      branch_nonce: number;
      scenario_spec?: {
        content_hash?: string;
        scenario_hash?: string;
        generator?: { specification_hash?: string };
      };
      field?: { now?: unknown };
      content_hash?: string;
    };
  };
  const runId = file.payload.run_id;
  const branchNonce = file.payload.branch_nonce;
  const id = `${runId}:${branchNonce}:${scenario.step}:${Date.now().toString(36)}`;
  const contentHash = file.payload.scenario_spec?.content_hash ?? file.payload.content_hash ?? scenario.id;
  const generatorHash = file.payload.scenario_spec?.generator?.specification_hash ?? contentHash;
  const scenarioHash = file.payload.scenario_spec?.scenario_hash ?? scenario.id;
  const embodiedHash = embodiedStateHash
    ?? compactHash(JSON.stringify(file.payload.field?.now ?? null));
  return {
    id,
    schemaVersion: 2,
    engineBuildHash: contentHash,
    lawsetVersion: regimeById(scenario.regime).lawset,
    protocolVersion: 2,
    contentHash,
    createdAt: Date.now(),
    runId,
    branchNonce,
    parentId: branchNonce > 0 ? `${runId}:${branchNonce - 1}` : null,
    parent: branchNonce > 0 ? { runId, branchNonce: branchNonce - 1, anchorId: null } : null,
    scenarioId: scenario.id,
    scenarioHash,
    regime: scenario.regime,
    form: scenario.form,
    embodiedStateHash: embodiedHash,
    generatorHash,
    inputHash: compactHash(JSON.stringify({ regime: scenario.regime, view: scenario.view, nodes: scenario.nodeIds, routes: scenario.routeIds })),
    controlHash: compactHash(JSON.stringify(scenario.controlContract)),
    rngAlgorithm: 'Philox2x64-10',
    reproducibilityStateKey: `${runId}:${branchNonce}:${scenario.step}`,
    estimatorVersion: null,
    analysisProtocolHash: compactHash(JSON.stringify(scenario.observation)),
    trialCount: evidence.reduce((largest, record) => Math.max(largest, record.trials), 0),
    criterionVector: [
      'route_throughput_floor',
      'component_charge_margin',
      'leakage_supply_ceiling',
      'failure_grace',
      'hands_off_duration',
    ],
    payloadBlobKey: id,
    control: scenario.control,
    step: scenario.step,
    payload: text,
    evidence,
  };
}
