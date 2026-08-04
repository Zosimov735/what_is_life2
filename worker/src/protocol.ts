/**
 * The worker protocol, as `docs/field-framework/ARCHITECTURE.md` locks it.
 *
 * The command set (32) and the event set (10) are closed for version 13: nothing
 * else crosses the boundary between the shell and the worker. Beside the
 * message layer — the envelopes, the closed name sets, and the error envelope —
 * this file re-declares the public core types one to one with the Rust core, in
 * the order the document locks them. The types the goals that own them have not
 * reached yet are declared by those goals.
 */

/** The protocol version field. */
export const PROTOCOL_VERSION = 13;

/** The save version this build reads and writes. */
export const SAVE_VERSION = 7;

/** The thirty-two commands the shell may send. */
export type CommandName =
  | 'list_contracts'
  | 'open_contract'
  | 'init_run'
  | 'input_frame'
  | 'queue_plan'
  | 'undo_plan'
  | 'commit_plan'
  | 'set_focus'
  | 'restore_checkpoint'
  | 'recover_branch'
  | 'export_run'
  | 'import_run'
  | 'reopen_archive'
  | 'run_analysis'
  | 'sample_instrument'
  | 'inspect_field'
  | 'compile_scenario'
  | 'run_scenario'
  | 'sample_lens'
  | 'renewal_trial'
  | 'renewal_inventory'
  | 'preview_design_patch'
  | 'commit_design_patch'
  | 'preview_commission_restart'
  | 'preview_qualification_input'
  | 'freeze_qualification_request'
  | 'qualification_job'
  | 'engineering_memory'
  | 'restart_commission'
  | 'return_commission'
  | 'resume_commission'
  | 'set_local_policy';

export type SupplySense = 'absent' | 'present' | 'emitting' | 'quiet';

export type LocalCondition =
  | { kind: 'always' }
  | { kind: 'charge_below' | 'charge_above'; fraction: Frac }
  | { kind: 'operating_margin_below'; amount: Fx }
  | { kind: 'supply'; state: SupplySense; radius: Fx }
  | { kind: 'target_in_range'; radius: Fx }
  | { kind: 'route_flow_below' | 'route_flow_above'; route: number; flow: Fx }
  | { kind: 'overloaded' }
  | { kind: 'signal_present'; radius: Fx }
  | { kind: 'timer_elapsed'; steps: number };

export type LocalAction =
  | { kind: 'hold' }
  | { kind: 'seek_supply' | 'seek_port' | 'seek_signal' | 'couple'; radius: Fx }
  | { kind: 'change_depth'; direction: -1 | 0 | 1 }
  | { kind: 'set_interface'; open: boolean }
  | {
      kind: 'set_route';
      route: number;
      enabled: boolean;
      capacity_limit: Fx;
      allocation_weight: number;
    }
  | { kind: 'emit_signal'; strength: Fx }
  | { kind: 'use_ability' };

export interface PolicyRule {
  enabled: boolean;
  condition: LocalCondition;
  action: LocalAction;
}

export interface ComponentPolicy {
  address: number;
  rules: PolicyRule[];
  fallback: LocalAction;
}

export interface FrozenLocalPolicy {
  version: 3;
  components: ComponentPolicy[];
}

/** One committed chapter-opening Route actuator setting. */
export interface RouteControlDefault {
  route: number;
  enabled: boolean;
  capacity_limit: Fx;
  allocation_weight: number;
  controller: number;
}

export type PolicyOutcome =
  | 'idle'
  | 'held'
  | 'applied'
  | 'no_target'
  | 'target_unavailable'
  | 'wrong_layer'
  | 'out_of_range'
  | 'no_effect'
  | 'cooldown'
  | 'capacity_reached'
  | 'unavailable';

export type CriterionStatus = 'active' | 'failed' | 'passed';

export type RunKind = 'automation_contract' | 'open_field' | 'legacy_campaign';
export type AttemptBranchOperation =
  | 'opening'
  | 'design_commit'
  | 'assembly_commit'
  | 'restart'
  | 'restart_assembly'
  | 'revert_generator'
  | 'full_contract_reset'
  | 'clone_blueprint'
  | 'rebranch'
  | 'resume'
  | 'migrated';

export interface CanonicalAttemptRecord {
  attempt_id: string;
  content_hash: string;
  contract_id: string;
  opening_assembly_hash: string;
  opening_generator_hash: string;
  source: 'opened' | 'migrated';
  version: 1;
}

export interface CanonicalAttemptBranchRecordV1 {
  assembly_hash: string;
  attempt_id: string;
  branch_id: string;
  branch_nonce: number;
  generator_hash: string;
  operation: AttemptBranchOperation;
  parent_branch_id: string | null;
  version: 1;
}

export interface CanonicalAttemptBranchRecordV2 extends Omit<CanonicalAttemptBranchRecordV1, 'version'> {
  transition_receipt: EngineeringRunTransitionReceipt | null;
  version: 2;
}

export type CanonicalAttemptBranchRecord =
  | CanonicalAttemptBranchRecordV1
  | CanonicalAttemptBranchRecordV2;

export interface RunLineage {
  assembly_template_exact: boolean;
  assembly_template_hash: string | null;
  attempt_branch: CanonicalAttemptBranchRecord | null;
  attempt_id: string | null;
  attempt_record: CanonicalAttemptRecord | null;
  branch_id: string | null;
  branch_nonce: number;
  branch_operation: AttemptBranchOperation | null;
  parent_branch_id: string | null;
  run_kind: RunKind;
}

export type PolicyTargetKind = 'none' | 'node' | 'route' | 'current' | 'signal';

export interface PolicyPreviewCandidate {
  distance: Fx;
  id: number | null;
  kind: PolicyTargetKind;
}

export interface PolicyPreview {
  action: LocalAction;
  action_radius: Fx;
  address: number;
  candidates: PolicyPreviewCandidate[];
  condition: LocalCondition | null;
  rule: number;
  sensor_radius: Fx;
  target: number | null;
  target_kind: PolicyTargetKind;
}

export interface DesignPreviewed extends Payload {
  base_generator_hash: string;
  preview: PolicyPreview;
  snapshot_step: number;
}

export interface DesignCommitted extends Payload, RunLineage {
  base_generator_hash: string;
  canonical_diff: {
    policy_changed: boolean;
    route_defaults_changed: number[];
  };
  control: string;
  generator_spec_hash: string;
  local_policy: FrozenLocalPolicy;
  route_defaults: RouteControlDefault[];
  scenario_hash: string;
}

/** Read-only authoritative consequences of restarting the current Commission branch. */
export interface CommissionRestartPreview extends Payload, RunLineage {
  assembly_template: Payload;
  assembly_template_exact: true;
  assembly_template_hash: string;
  branch_id: string;
  boundary: 'contract_opening';
  consequences: {
    create_child_branch: true;
    keep_generator: true;
    restore_assembly: true;
    retain_evidence: true;
  };
  content_hash: string;
  contract_id: string;
  current_embodied_state_hash: string;
  current_step: number;
  generator_spec: Payload;
  generator_spec_hash: string;
  predicted_branch_nonce: number;
  predicted_operation: 'restart';
  predicted_parent_branch_id: string;
  preview_version: 1;
  regime: string;
  scenario_hash: string;
}

export type QualificationPreviewStatus = 'complete' | 'incomplete';

export interface QualificationInput extends Payload, RunLineage {
  assembly_template: Payload;
  assembly_template_hash: string;
  build: { package: string; version: string };
  content_hash: string;
  contract_id: string;
  criterion_vector: {
    criteria: ContractCriterion[];
    failure_grace_steps: number;
    version: 1;
  };
  criterion_vector_hash: string;
  generator_spec: Payload;
  generator_spec_hash: string;
  grade_axes: Record<'throughput' | 'resilience' | 'economy' | 'complexity', {
    bands: [Frac, Frac, Frac, Frac];
    evidence: string;
  }>;
  missing_inputs: string[];
  procedure: {
    control_contract: 'hands_off';
    early_resolution: 'none';
    progress_interval_steps: number;
    retention: 'criterion_windows_first_violation_terminal';
    rng_algorithm: 'philox4x32_10_v1';
    schedule: Payload;
    schedule_hash: string;
    seed_custody: 'request_hash_and_trial_address';
    suite_version: 1;
    trial_addresses: Array<{ trial: number }>;
    trial_count: number;
  };
  prospective_receipt: ContractCapabilitySet & { next_contract: string | null };
  protocol_version: number;
  regime: RegimeId;
  scenario_hash: string;
  schema_version: 1;
}

/** Complete read-only input bundle that Q-01 can freeze without shell defaults. */
export interface QualificationInputPreview extends Payload {
  input: QualificationInput;
  missing_inputs: string[];
  preview_hash: string;
  preview_version: 1;
  status: QualificationPreviewStatus;
}

/** Immutable Q-01 authority stored in the V6 save and browser archive. */
export interface CanonicalQualificationRequest {
  input: QualificationInput;
  request_id: string;
  version: 1;
}

/** The accepted freeze response. Q-02 may consume only its request id. */
export interface QualificationFrozen extends Payload, RunLineage {
  content_hash: string;
  contract_id: string;
  embodied_state_hash: string;
  generator_spec_hash: string;
  input: QualificationInput;
  qualification_request: CanonicalQualificationRequest;
  qualification_request_id: string;
  request_id: string;
  scenario_hash: string;
  status: 'frozen_pending_persistence';
}

export type QualificationJobStatus =
  | 'queued'
  | 'running'
  | 'cancel_requested'
  | 'canceled'
  | 'completed'
  | 'interrupted'
  | 'invalid_execution';

export interface QualificationJob extends Payload {
  completed_trials: number[];
  duration_steps: number;
  job_id: string;
  progress_interval_steps: number;
  request_id: string;
  status: QualificationJobStatus;
  trial_count: number;
  version: 3;
}

export interface QualificationTrialArtifact extends Payload {
  artifact_id: string;
  criterion_runtime: Payload | null;
  duration_steps: number;
  executed_steps: number;
  first_failure_events: Payload[];
  first_failure_events_truncated: boolean;
  first_failure_payload: string | null;
  first_failure_payload_hash: string | null;
  first_failure_step: number | null;
  grade_evidence: QualificationTrialGradeEvidence;
  job_id: string;
  request_id: string;
  status: 'completed';
  terminal_embodied_state_hash: string;
  terminal_events: Payload[];
  terminal_events_truncated: boolean;
  terminal_payload: string;
  terminal_payload_hash: string;
  trial: number;
  version: 3;
}

export interface QualificationTrialGradeEvidence extends Payload {
  drain: number;
  final_material_units: number;
  initial_material_units: number;
  interventions: number;
  leakage: number;
  materials: Array<{
    final: number;
    initial: number;
    kind: 'boundary_blank' | 'conductor' | 'junction_blank';
  }>;
  moved: number;
  overload: number;
  renewal: number;
  supply: number;
  upkeep: number;
  version: 1;
}

export interface QualificationProgress extends Payload {
  artifact: QualificationTrialArtifact | null;
  completed_trials: number[];
  current_trial: number | null;
  job_id: string;
  request_id: string;
  status: QualificationJobStatus;
  trial_count: number;
}

export interface QualificationCriterionDecisionDefinition extends Payload {
  aggregation: ContractCriterionAggregation;
  artifact_id: string;
  comparison: ContractCriterionComparison;
  criterion_id: string;
  job_id: string;
  margin: number;
  measured: number;
  metric: ContractCriterionMetric;
  passed: boolean;
  request_id: string;
  resolution_step: number;
  source: ContractCriterion['source'];
  status: 'passed' | 'failed';
  threshold: number;
  trial: number;
  version: 1;
  window_end_step: number;
  window_start_step: number;
  window_steps: number;
}

export interface QualificationCriterionDecision extends Payload {
  decision_id: string;
  definition: QualificationCriterionDecisionDefinition;
}

export interface QualificationFunctionDecisionDefinition extends Payload {
  criterion_decision_ids: string[];
  job_id: string;
  passed: boolean;
  request_id: string;
  status: 'passed' | 'failed';
  trial_count: number;
  version: 1;
}

export interface QualificationFunctionDecision extends Payload {
  definition: QualificationFunctionDecisionDefinition;
  function_decision_id: string;
}

export interface QualificationResolution extends Payload {
  criterion_decisions: QualificationCriterionDecision[];
  function_decision: QualificationFunctionDecision;
  job_id: string;
  request_id: string;
  status: 'resolved';
  version: 1;
}

export type QualificationGradeAxis = 'throughput' | 'resilience' | 'economy' | 'complexity';

export interface QualificationGradeDefinition extends Payload {
  axis: QualificationGradeAxis;
  band: 0 | 1 | 2 | 3 | 4;
  band_definition_hash: string;
  bands: [Frac, Frac, Frac, Frac];
  evidence: Payload;
  function_decision_id: string;
  job_id: string;
  request_id: string;
  score: Frac;
  status: 'available';
  version: 1;
}

export interface QualificationGrade extends Payload {
  definition: QualificationGradeDefinition;
  grade_id: string;
}

export interface QualificationGrades extends Payload {
  grades: QualificationGrade[];
  job_id: string;
  request_id: string;
  status: 'graded';
  version: 1;
}

export interface QualificationFailureTraceDefinition extends Payload {
  artifact_id: string;
  criterion_decision_id: string;
  function_decision_id: string;
  inference_algorithm: 'direct_records_only_v1';
  inferred_contributors: Payload[];
  job_id: string;
  mechanism_events: Payload[];
  payload_hash: string;
  request_id: string;
  resolution_step: number;
  source: ContractCriterion['source'];
  status: 'complete' | 'incomplete';
  trace_keyframe_hash: string;
  trace_start_step: number;
  trace_steps: Payload[];
  trial: number;
  version: 1;
  window_start_step: number;
}

export interface QualificationFailureTrace extends Payload {
  definition: QualificationFailureTraceDefinition;
  failure_trace_id: string;
}

export interface QualificationFailureTraceResult extends Payload {
  failure_trace: QualificationFailureTrace | null;
  job_id: string;
  request_id: string;
  status: 'traced' | 'not_applicable';
  version: 1;
}

export interface QualificationResultDefinition extends Payload {
  artifact_ids: string[];
  assembly_template_hash: string;
  build: { package: string; version: string };
  content_hash: string;
  contract_id: string;
  criterion_decision_ids: string[];
  execution_status: 'completed';
  failure_trace_id: string | null;
  function_decision_id: string;
  generator_spec_hash: string;
  grade_ids: string[];
  job_id: string;
  outcome: 'passed' | 'failed';
  protocol_version: number;
  request_id: string;
  scenario_hash: string;
  trial_count: number;
  version: 1;
}

export interface QualificationResult extends Payload {
  definition: QualificationResultDefinition;
  result_id: string;
}

export interface QualificationCompleteMarkerDefinition extends Payload {
  child_count: number;
  result_id: string;
  version: 1;
}

export interface QualificationCompleteMarker extends Payload {
  definition: QualificationCompleteMarkerDefinition;
  marker_id: string;
}

export interface QualificationResultGroup extends Payload {
  complete_marker: QualificationCompleteMarker;
  result: QualificationResult;
  status: 'complete';
  version: 1;
}

export interface QualificationUnlockReceiptDefinition extends Payload {
  actions: LocalAction['kind'][];
  conditions: LocalCondition['kind'][];
  content_hash: string;
  contract_id: string;
  hardware: string[];
  next_contract: string | null;
  prerequisites: string[];
  result_id: string;
  version: 1;
}

export interface QualificationUnlockReceipt extends Payload {
  definition: QualificationUnlockReceiptDefinition;
  receipt_id: string;
}

export interface QualificationReceiptResult extends Payload {
  receipt: QualificationUnlockReceipt;
  status: 'derived';
  version: 1;
}

export interface EngineeringRecordSource extends Payload {
  attempt_id: string;
  authority: 'committed_design' | 'qualification_result' | 'migrated_v1';
  branch_id: string;
  result_id: string | null;
  version: 1;
}

export interface EngineeringDerivationEdge extends Payload {
  operation: 'capture' | 'promote' | 'clone' | 'hypothesis_branch' | 'transplant' | 'assembly_adaptation';
  source_id: string;
  source_kind: 'attempt_branch' | 'blueprint' | 'qualification_request' | 'qualification_result' | 'legacy_blueprint';
  version: 1;
}

export interface EngineeringEvidenceLink extends Payload {
  availability: 'available' | 'unavailable';
  evidence_id: string;
  evidence_kind: 'qualification_request' | 'qualification_result' | 'failure_trace' | 'comparative_qualification';
  role: 'source_qualification' | 'diagnostic' | 'comparison_source' | 'first_failure';
  version: 1;
}

export interface EngineeringAssemblyRecordDefinitionV1 extends Payload {
  assembly_template: Payload;
  assembly_template_hash: string;
  compatibility: {
    contract_id: string;
    regime: RegimeId;
    run_kind: 'automation_contract';
    version: 1;
  };
  migration: {
    source: 'exact_runtime_opening';
    source_version: 1;
  };
  version: 1;
}

export interface EngineeringAssemblyRecordDefinitionV2 extends Payload {
  assembly_template: Payload;
  assembly_template_hash: string;
  compatibility: {
    content_hash: string;
    contract_id: string;
    generator_record_id: string;
    regime: RegimeId;
    run_kind: 'automation_contract';
    version: 2;
  };
  owned_fields: [
    'component_opening_state',
    'component_placement',
    'current_phase',
    'form_reserve',
    'interface_state',
    'material_placement_and_stock',
    'physical_compartment',
    'stored_charge',
  ];
  protocol_version: number;
  source: EngineeringRecordSource;
  version: 2;
}

export type EngineeringAssemblyRecordDefinition =
  | EngineeringAssemblyRecordDefinitionV1
  | EngineeringAssemblyRecordDefinitionV2;

export interface EngineeringAssemblyRecord extends Payload {
  assembly_record_id: string;
  definition: EngineeringAssemblyRecordDefinition;
}

export interface EngineeringGeneratorRecordDefinitionV1 extends Payload {
  generator_spec: Payload;
  generator_spec_hash: string;
  version: 1;
}

export interface EngineeringGeneratorRecordDefinitionV2 extends Payload {
  content_hash: string;
  contract_id: string;
  generator_spec: Payload;
  generator_spec_hash: string;
  protocol_version: number;
  source: EngineeringRecordSource;
  version: 2;
}

export type EngineeringGeneratorRecordDefinition =
  | EngineeringGeneratorRecordDefinitionV1
  | EngineeringGeneratorRecordDefinitionV2;

export interface EngineeringGeneratorRecord extends Payload {
  definition: EngineeringGeneratorRecordDefinition;
  generator_record_id: string;
}

export interface BlueprintRecordDefinitionV1 extends Payload {
  assembly_record_id: string;
  branch_id: string;
  contract_id: string;
  content_hash: string;
  generator_record_id: string;
  linked_result_ids: string[];
  source_attempt_id: string;
  version: 1;
}

export interface BlueprintRecordDefinitionV2 extends Payload {
  assembly_record_id: string;
  content_hash: string;
  contract_id: string;
  creation_reason: 'design_capture' | 'result_capture' | 'v1_promotion' | 'clone' | 'transplant';
  derivation_edges: EngineeringDerivationEdge[];
  evidence_links: EngineeringEvidenceLink[];
  generator_record_id: string;
  parent_blueprint_id: string | null;
  protocol_version: number;
  source_attempt_id: string;
  source_branch_id: string;
  version: 2;
}

export type BlueprintRecordDefinition = BlueprintRecordDefinitionV1 | BlueprintRecordDefinitionV2;

export interface BlueprintRecord extends Payload {
  blueprint_id: string;
  definition: BlueprintRecordDefinition;
}

export interface EngineeringMemoryCaptureV1 extends Payload {
  assembly_record: EngineeringAssemblyRecord;
  blueprint: BlueprintRecord;
  generator_record: EngineeringGeneratorRecord;
  status: 'captured';
  version: 1;
}

export interface EngineeringMemoryCaptureV2 extends Payload {
  assembly_record: EngineeringAssemblyRecord;
  blueprint: BlueprintRecord;
  generator_record: EngineeringGeneratorRecord;
  status: 'captured';
  version: 2;
}

export type EngineeringMemoryCapture = EngineeringMemoryCaptureV1 | EngineeringMemoryCaptureV2;

export type EngineeringCaptureSource =
  | { kind: 'committed_design'; result_id: null }
  | { kind: 'qualification_result'; result_id: string };

export interface EngineeringAssemblyDraft extends Payload {
  components: Array<{
    layer: number;
    node: number;
    open: boolean;
    pos: Vec2;
    q: Fx;
  }>;
  currents: Array<{
    active: boolean;
    current: number;
    phase: number;
  }>;
  forms: Array<{
    junction_blanks: number | null;
    node: number;
    reserve: Fx;
  }>;
  materials: Array<{
    amount: number;
    layer: number;
    material: number;
    pos: Vec2;
  }>;
  physical_compartment: PhysicalCompartmentState;
  version: 1;
}

export interface EngineeringAssemblyDiffChange extends Payload {
  address: string;
  after: Payload;
  before: Payload;
  kind: 'component' | 'current' | 'form' | 'material' | 'physical_compartment';
  version: 1;
}

export interface EngineeringAssemblyDiff extends Payload {
  definition: {
    after_assembly_hash: string;
    before_assembly_hash: string;
    changes: EngineeringAssemblyDiffChange[];
    version: 1;
  };
  diff_id: string;
}

export interface EngineeringAssemblyDraftResult extends Payload, RunLineage {
  assembly_draft: EngineeringAssemblyDraft;
  generator_spec_hash: string;
  status: 'ready';
  version: 1;
}

export interface EngineeringAssemblyWarning extends Payload {
  address: string | null;
  code: 'route_defaults_reapplied' | 'live_claims_cleared' | 'opening_state_normalized';
  version: 1;
}

export interface EngineeringAssemblyCompatibility extends Payload {
  assembly_owned_only: true;
  generator_unchanged: true;
  issues: [];
  status: 'compatible';
  version: 1;
}

export interface EngineeringAssemblyPreview extends Payload, RunLineage {
  candidate_assembly_hash: string;
  candidate_assembly_template: Payload;
  candidate_draft: EngineeringAssemblyDraft;
  compatibility: EngineeringAssemblyCompatibility;
  diff: EngineeringAssemblyDiff;
  generator_spec_hash: string;
  preview_id: string;
  status: 'accepted';
  version: 1;
  warnings: EngineeringAssemblyWarning[];
}

export interface EngineeringAssemblyCommitResult extends Payload, RunLineage {
  assembly_record: EngineeringAssemblyRecord;
  diff: EngineeringAssemblyDiff;
  generator_record: EngineeringGeneratorRecord;
  previous_assembly_hash: string;
  previous_branch_id: string;
  preview_id: string;
  status: 'committed';
  transition_receipt: EngineeringAssemblyCommitReceipt;
  version: 1;
}

export type EngineeringIdentityKind =
  | 'attempt'
  | 'branch'
  | 'generator'
  | 'assembly'
  | 'qualification_request'
  | 'qualification_result'
  | 'blueprint';

export interface EngineeringIdentityDisposition extends Payload {
  disposition: 'retained' | 'restored' | 'recreated' | 'detached';
  identity: string;
  kind: EngineeringIdentityKind;
  version: 1;
}

export interface EngineeringAssemblyCommitReceipt extends Payload {
  child_attempt_id: string;
  child_branch_id: string;
  identities: EngineeringIdentityDisposition[];
  operation: 'assembly_commit';
  operation_id: string;
  parent_attempt_id: string;
  parent_branch_id: string;
  preview_id: string;
  reconstruction_digest: string;
  recovery_state: 'accepted_unpersisted' | 'persisted' | 'recovered';
  version: 1;
}

export type EngineeringTransitionKind =
  | 'restart_assembly'
  | 'revert_generator'
  | 'full_contract_reset';

export type EngineeringTransitionSource =
  | { kind: 'current_committed'; source_id: null; version: 1 }
  | { kind: 'generator_record'; source_id: string; version: 1 }
  | { kind: 'authored_contract_opening'; source_id: string; version: 1 };

export interface EngineeringTransitionGuard extends Payload {
  assembly_hash: string;
  attempt_id: string;
  branch_id: string;
  branch_nonce: number;
  content_hash: string;
  contract_id: string;
  embodied_hash: string;
  generator_hash: string;
  lifecycle: 'still';
  run_kind: 'automation_contract';
  scenario_hash: string;
  version: 2;
}

export type EngineeringTransitionRegisterKind =
  | 'live_positions'
  | 'stored_charge'
  | 'policy_timers'
  | 'controller_state'
  | 'event_window'
  | 'provisional_criteria';

export interface EngineeringTransitionRegisterConsequence extends Payload {
  addresses: string[];
  after_digest: string;
  after_disposition: 'recreated';
  before_digest: string;
  before_disposition: 'detached';
  kind: EngineeringTransitionRegisterKind;
  version: 1;
}

export type EngineeringAssemblyCompatibilityFieldKind =
  | 'component_position'
  | 'component_layer'
  | 'stored_charge'
  | 'interface_state'
  | 'form_reserve'
  | 'junction_blanks'
  | 'material_amount'
  | 'material_position'
  | 'material_layer'
  | 'current_active'
  | 'current_phase'
  | 'physical_compartment_members'
  | 'physical_compartment_leakage';

export type EngineeringAssemblyCompatibilityDisposition =
  | 'retained_unchanged'
  | 'retained_by_address'
  | 'adaptation_required'
  | 'hard_refusal';

export interface EngineeringAssemblyCompatibilityField extends Payload {
  address: string;
  after_digest: string;
  before_digest: string;
  disposition: EngineeringAssemblyCompatibilityDisposition;
  field: EngineeringAssemblyCompatibilityFieldKind;
  issue_code: EngineeringCompatibilityIssue['code'] | null;
  version: 1;
}

export interface EngineeringRunTransitionPreviewDefinition extends Payload {
  commit_allowed: boolean;
  compatibility_fields: EngineeringAssemblyCompatibilityField[];
  compatibility_issues: EngineeringCompatibilityIssue[];
  current_regime_id: RegimeId;
  guard: EngineeringTransitionGuard;
  identities: EngineeringIdentityDisposition[];
  operation: EngineeringTransitionKind;
  reconstruction_digest: string;
  registers: EngineeringTransitionRegisterConsequence[];
  source: EngineeringTransitionSource;
  target_assembly_draft: EngineeringAssemblyDraft;
  target_assembly_hash: string;
  target_generator_hash: string;
  target_regime_id: RegimeId;
  target_scenario_hash: string;
  version: 3;
}

export interface EngineeringRunTransitionPreview extends Payload {
  definition: EngineeringRunTransitionPreviewDefinition;
  preview_id: string;
}

export interface EngineeringTransitionPreviewAccepted extends Payload {
  preview: EngineeringRunTransitionPreview;
  status: 'accepted';
  version: 5;
}

export type EngineeringTransitionRefusalCode =
  | 'wrong_run_kind'
  | 'wrong_lifecycle'
  | 'stale_contract'
  | 'stale_attempt'
  | 'stale_branch'
  | 'stale_generator'
  | 'stale_assembly'
  | 'stale_preview'
  | 'source_unavailable'
  | 'source_corrupt'
  | 'incompatible_assembly'
  | 'reconstruction_failed'
  | 'qualification_frozen'
  | 'unsupported_operation';

export interface EngineeringTransitionRefused extends Payload {
  code: EngineeringTransitionRefusalCode;
  field: string | null;
  operation: EngineeringTransitionKind;
  status: 'refused';
  version: 1;
}

export interface EngineeringRunTransitionReceipt extends Payload {
  after_assembly_hash: string;
  after_generator_hash: string;
  after_regime_id?: RegimeId;
  after_scenario_hash?: string;
  before_assembly_hash: string;
  before_generator_hash: string;
  before_regime_id?: RegimeId;
  before_scenario_hash?: string;
  child_attempt_id: string;
  child_branch_id: string;
  closure_reason: 'restart' | 'superseded';
  compatibility_fields?: EngineeringAssemblyCompatibilityField[];
  compatibility_issues: EngineeringCompatibilityIssue[];
  detached_evidence_ids: string[];
  identities: EngineeringIdentityDisposition[];
  operation: EngineeringTransitionKind;
  operation_id: string;
  parent_attempt_id: string;
  parent_branch_id: string;
  preview_id: string;
  reconstruction_digest: string;
  registers: EngineeringTransitionRegisterConsequence[];
  recovery_state:
    | 'accepted_unpersisted'
    | 'prior_retained'
    | 'child_published'
    | 'pointer_moved'
    | 'persisted'
    | 'recovered';
  source: EngineeringTransitionSource;
  version: 3 | 4 | 5;
}

export type EngineeringTransitionReceipt =
  | EngineeringAssemblyCommitReceipt
  | EngineeringRunTransitionReceipt;

export interface EngineeringTransitionCommitResult extends Payload, RunLineage {
  assembly_record: EngineeringAssemblyRecord;
  generator_record: EngineeringGeneratorRecord;
  preview_id: string;
  regime: RegimeId;
  scenario_hash: string;
  status: 'committed';
  transition_receipt: EngineeringRunTransitionReceipt;
  version: 3 | 4 | 5;
}

export interface EngineeringTransitionRecoveryResult extends Payload, RunLineage {
  regime: RegimeId;
  scenario_hash: string;
  status: 'recovered';
  transition_receipt: EngineeringRunTransitionReceipt;
  version: 3 | 4 | 5;
}

export interface EngineeringDiffRow extends Payload {
  address: string;
  after: Payload | null;
  before: Payload | null;
  kind: 'equal' | 'added' | 'removed' | 'changed' | 'unaligned' | 'unavailable';
  section: 'generator_design' | 'initial_assembly' | 'observed_evidence';
  version: 1;
}

export interface EngineeringDiffReport extends Payload {
  definition: {
    left_id: string;
    left_kind: 'blueprint' | 'branch' | 'qualification_request' | 'qualification_result';
    normalization_version: 1;
    right_id: string;
    right_kind: 'blueprint' | 'branch' | 'qualification_request' | 'qualification_result';
    rows: EngineeringDiffRow[];
    version: 1;
  };
  diff_id: string;
}

export interface EngineeringCompatibilityIssue extends Payload {
  address: string | null;
  code:
    | 'missing_hardware'
    | 'unsupported_action'
    | 'unsupported_condition'
    | 'invalid_address'
    | 'invalid_route_ownership'
    | 'missing_material'
    | 'regime_incompatible_assembly'
    | 'generator_edit_required';
  disposition: 'hard_incompatibility' | 'assembly_adaptation' | 'generator_edit_required';
  version: 1;
}

export interface EngineeringCompatibilityReport extends Payload {
  compatibility_id: string;
  definition: {
    destination_contract_id: string;
    destination_regime: RegimeId;
    issues: EngineeringCompatibilityIssue[];
    source_assembly_record_id: string;
    source_generator_record_id: string;
    unchanged_generator: boolean;
    version: 1;
  };
}

export interface EngineeringAssemblyAdaptationRecord extends Payload {
  adaptation_id: string;
  definition: {
    compatibility_id: string;
    destination_assembly_record_id: string;
    destination_branch_id: string;
    source_assembly_record_id: string;
    unchanged_generator_record_id: string;
    version: 1;
  };
}

export interface ComparativeQualificationRecord extends Payload {
  comparative_id: string;
  definition: {
    comparability: 'comparable' | 'qualified_with_uncontrolled_differences' | 'incomparable';
    controlled_differences: string[];
    destination_result_id: string;
    first_divergence: string | null;
    matched_identities: string[];
    source_result_id: string;
    uncontrolled_differences: string[];
    version: 1;
  };
}

export type ContractStatus = 'available' | 'locked' | 'completed';
export type ContractCriterionMetric =
  | 'stored_charge'
  | 'accepted_flow'
  | 'leakage_ratio'
  | 'hands_off_steps';
export type ContractCriterionComparison = 'at_least' | 'at_most';
export type ContractCriterionAggregation = 'minimum' | 'maximum' | 'final';

export interface ContractCriterion {
  aggregation: ContractCriterionAggregation;
  comparison: ContractCriterionComparison;
  id: string;
  metric: ContractCriterionMetric;
  source: { id: number | null; kind: 'field' | 'component' | 'route' };
  threshold: number;
  window_steps: number;
}

export interface ContractCapabilitySet {
  actions: LocalAction['kind'][];
  conditions: LocalCondition['kind'][];
  hardware: string[];
}

export interface ContractCatalogEntry {
  available: boolean;
  brief_key: string;
  capabilities: ContractCapabilitySet;
  commissioning: {
    expected_minutes: number;
    maximum_wall_wait_seconds: number;
  };
  criteria: ContractCriterion[];
  failure_key: string;
  grade_bands: {
    complexity: [Frac, Frac, Frac, Frac];
    economy: [Frac, Frac, Frac, Frac];
    resilience: [Frac, Frac, Frac, Frac];
    throughput: [Frac, Frac, Frac, Frac];
  };
  guidance_keys: string[];
  id: string;
  limits: {
    max_components: number;
    max_routes: number;
    max_rules_per_component: number;
  };
  missing_prerequisites: string[];
  opening: {
    assembly_template_hash: string;
    component_count: number;
    form: FormId;
    generator_spec_hash: string;
    regime: RegimeId;
    route_count: number;
    supply_cycles: Array<{
      current: number;
      duty: Frac;
      on_steps: number;
      period: number;
    }>;
  };
  order: number;
  prerequisites: string[];
  qualification: {
    duration_steps: number;
    failure_grace_steps: number;
    trial_count: number;
  };
  status: ContractStatus;
  success_key: string;
  title_key: string;
  unlocks: ContractCapabilitySet & {
    next_contract: string | null;
  };
}

export interface ContractCatalog extends Payload {
  contract_version: 2;
  contracts: ContractCatalogEntry[];
}

/** The ten events the worker may raise unsolicited. */
export type EventName =
  | 'frame'
  | 'mechanism_event'
  | 'criterion_changed'
  | 'objective_changed'
  | 'pressure_changed'
  | 'review_ready'
  | 'checkpoint_written'
  | 'chapter_changed'
  | 'run_completed'
  | 'qualification_progress';

/** The closed error code set. */
export type ErrorCode =
  | 'protocol'
  | 'state'
  | 'validation'
  | 'impulse'
  | 'capacity'
  | 'not_found'
  | 'save_corrupt'
  | 'save_version'
  | 'import_invalid'
  | 'content_invalid'
  | 'worker_restart'
  | 'internal';

/** Structured-clone-safe data: the only shape a message body may carry. */
export type Payload = Record<string, unknown>;

/**
 * A Q32.16 binary fixed-point quantity, raw: the value times 65536. Every
 * simulation quantity crosses as one of these, as a plain JSON integer, because
 * no floating-point number ever appears in a protocol body.
 */
export type Fx = number;

/** `Fx` restricted to [0, 65536]: the raw form of [0, 1]. */
export type Frac = number;

/** A position on a layer plane, raw per axis, inside [0, 4096) units. */
export interface Vec2 {
  x: Fx;
  y: Fx;
}

/** The closed Node-kind set of version 2. */
export type NodeKind = 'port' | 'reserve' | 'module' | 'form';

/** The eight starting Forms, as their machine ids. */
export type FormId =
  | 'thread'
  | 'ring'
  | 'relay'
  | 'vault'
  | 'lens'
  | 'knot'
  | 'wake'
  | 'chorus';

/**
 * The same eight, in the closed set's own order, for a surface that has to
 * offer them. The order is the set's and is not a ranking: nothing here, and
 * nothing that reads it, marks one Form as the one to take.
 */
export const FORM_IDS: readonly FormId[] = [
  'thread',
  'ring',
  'relay',
  'vault',
  'lens',
  'knot',
  'wake',
  'chorus',
];

/**
 * One layer and its three authored difficulty parameters: `drain` is Charge
 * removed per step by depth, `noise` distorts routes and forecasts, and `gain`
 * scales rewards. Lists ascending.
 */
export interface FieldLayer {
  layer: number;
  drain: Fx;
  noise: Frac;
  gain: Frac;
  current_ids: number[];
  port_ids: number[];
}

/** One Form. Every Form is also a Node, which `node` binds it to. */
export interface FormState {
  id: number;
  form: FormId;
  node: number;
  controlled: boolean;
  layer: number;
  pos: Vec2;
  /** In units per step. */
  vel: Vec2;
  charge: Fx;
  reserve: Fx;
  pulse_charge: Frac;
  focus: boolean;
  route_reach: Fx;
  /** What a Route this Form forms carries per step. */
  route_capacity: Fx;
  forecast_depth: number;
  /** The steering reach scale this Form was authored with, [16384, 262144]. */
  steer_scale: Frac;
  /** The station a linked Form holds, and the distance past which it stands separated. */
  link: { offset: Vec2; separation: Fx } | null;
  /** What a Form authoring a Trail leaves, and what a left entry does when it comes due. */
  trail: { period: number; delay: number; radius: Fx; magnitude: Fx } | null;
}

/** One deposited Trail entry, standing until the step it falls due on. */
export interface PendingTrail {
  form: number;
  layer: number;
  pos: Vec2;
  due: number;
  magnitude: Fx;
}

/** One current: an authored polyline flow, `strength` in Charge per step. */
export interface CurrentState {
  id: number;
  layer: number;
  path: Vec2[];
  width: Fx;
  strength: Fx;
  /** Fraction of each period that physically emits; strength is the cycle mean. */
  duty: Frac;
  period: number;
  phase: number;
  bright: boolean;
  active: boolean;
}

/**
 * One Port, which is a Node of the Field. `q` is stored Charge for the completed
 * step, `upkeep_rate` is Charge per step the Node pays to keep participating,
 * and `capacity` is the overload threshold.
 */
export interface PortState {
  node: number;
  layer: number;
  pos: Vec2;
  kind: NodeKind;
  q: Fx;
  open: boolean;
  upkeep_rate: Fx;
  capacity: Fx;
}

/** One Route, directed tail to head. `capacity` caps the per-step flow. */
export interface RouteState {
  route: number;
  tail: number;
  head: number;
  capacity: Fx;
  flow: Fx;
  formed_step: number;
}

/**
 * Candidate-Boundary seeds only — drawn most recent first and authored in
 * authored order. These never determine leakage or physical membership.
 */
export interface BoundaryState {
  drawn: { members: number[]; step: number }[];
  authored: { members: number[] }[];
}

/** The causal material compartment, independent of the observation View. */
export interface PhysicalCompartmentState {
  members: number[];
  leak_per_exposed_contact_per_step: Frac;
}

/** The six pressures of the closed set, in ordinal order. */
export const PRESSURE_IDS = [
  'drain',
  'noise',
  'fracture',
  'flood',
  'interference',
  'drift',
] as const;

export type PressureId = (typeof PRESSURE_IDS)[number];

/** The four stages a pressure passes through, in order. */
export const PRESSURE_STAGES = ['signal', 'pressure', 'crisis', 'resolution'] as const;

export type PressureStage = (typeof PRESSURE_STAGES)[number];

/** What a pressure is aimed at. */
export interface PressureTarget {
  t: 'none' | 'node' | 'route' | 'layer';
  id: number | null;
}

/** The Pulse's pressed-back floor, read while the stage matches its record. */
export interface PressureDisplaced {
  stage: PressureStage;
  level: number;
}

/** Flood's held working target, null everywhere else. */
export interface PressureBound {
  t: 'none' | 'node' | 'route' | 'layer';
  id: number;
}

/** One staged pressure, exactly as ARCHITECTURE.md locks it. */
export interface PressureState {
  pressure: PressureId;
  stage: PressureStage;
  /** The stage machine's curve value for the completed step, raw Q0.16. */
  level: number;
  primary: boolean;
  queued: boolean;
  start_step: number;
  target: PressureTarget;
  displaced: PressureDisplaced | null;
  bound: PressureBound | null;
}

/** The `pressure_changed` body: the full list after the change. */
export interface PressureChanged {
  pressures: PressureState[];
}

/** The three closed surround rules. */
export type Surround = 'adjacent' | 'double' | 'whole';

/** The View tuple, unchanged from the framework. */
export interface ViewDeclaration {
  inside: number[];
  resolution: number;
  window: number;
  surround: Surround;
}

/** The implemented Atlas regimes visible in the Number 2 shell. */
export type RegimeId =
  | 'open_field'
  | 'periodic_transport'
  | 'crowded_medium'
  | 'vestige_pressure'
  | 'holdout_atmosphere';

/** A passive measurement contract. It is never part of a causal plan. */
export interface ObservationProtocol extends Payload {
  instrument:
    | 'stored_charge'
    | 'route_flow'
    | 'view_boundary_flow'
    | 'supply_uptake'
    | 'physical_leakage'
    | 'maintenance_allocation'
    | 'initial_stock_estimate'
    | 'response_lag';
  resolution: number;
  window: number;
  surround: Surround;
}

/** The target-specific Intervention Bench union. */
export type InterventionPlan =
  | { tool: 'blade'; scope: 'replay' | 'live'; route: number; onset: number }
  | { tool: 'clamp'; scope: 'replay' | 'live'; route: number; inhibition: Frac; onset: number; duration: number }
  | { tool: 'scramble'; scope: 'replay' | 'live'; network: number[]; probability: Frac; onset: number; duration: number }
  | { tool: 'decoy'; scope: 'replay' | 'live'; supply: number; receiving_node: number; capture: Frac; onset: number; duration: number }
  | { tool: 'delay'; scope: 'replay' | 'live'; input: number; delay: number; onset: number }
  | { tool: 'replace'; scope: 'replay' | 'live'; components: number[]; fraction: Frac; transferred: string[]; onset: number }
  | { tool: 'breach'; scope: 'replay' | 'live'; member: number; coefficient_delta: Frac; onset: number; duration: number }
  | { tool: 'transplant'; scope: 'replay' | 'live'; regime: RegimeId; equilibration: number; retained: string[] };

/** Everything frozen before an Ensemble or Holdout family begins. */
export interface AnalysisScenarioSpec extends Payload {
  scenario_id: string;
  regime: RegimeId;
  generator_hash: string;
  initial_state_hash: string;
  control_mode: 'recorded_open_loop' | 'frozen_policy' | 'hands_off';
  view: ViewDeclaration;
  observation: ObservationProtocol;
  intervention: InterventionPlan | null;
  seeds: number[];
  sealed: boolean;
}

export type AnalysisJobKind =
  | 'divergence'
  | 'ensemble'
  | 'holdout'
  | 'archive'
  | 'renewal'
  | 'inheritance'
  | 'compile_scenario';
export type AnalysisJobStatus = 'queued' | 'running' | 'complete' | 'cancelled' | 'failed';

/** Cold-path analysis crosses as one job summary, never as streamed states. */
export interface AnalysisJob extends Payload {
  job_id: string;
  kind: AnalysisJobKind;
  status: AnalysisJobStatus;
  scenario_id: string;
  completed: number;
  total: number;
  result: Payload | null;
}

/** One stream position: a key and counter as fixed-width hex, and the half. */
export interface RngState {
  key: string;
  ctr: string;
  half: number;
}

/** The two kinds of record a run writes. */
export type RecordKind = 'anchor' | 'auto';

/**
 * Anchor metadata. The payload lives in the persistence record `save_key`
 * names, and `rng` is the trajectory position at the write, so a Quick Retry
 * restores the exact random state.
 */
export interface CheckpointState {
  anchor_id: number;
  step: number;
  chapter_index: number;
  objective_id: string;
  kind: RecordKind;
  save_key: string;
  rng: RngState;
  branch_nonce: number;
}

/** The four states an objective's `state` field carries. */
export type ObjectiveStage = 'hidden' | 'active' | 'complete' | 'failed_recoverable';

/**
 * The single visible objective's state. `id` is the objective's copy-catalog
 * key and is the empty string exactly while `state` is `hidden` and none has
 * been offered; at most one objective is active at a time.
 */
export interface ObjectiveState {
  id: string;
  state: ObjectiveStage;
  progress: Frac;
  target: Fx | null;
  started_step: number;
  completed_step: number | null;
}

export interface CriterionReading {
  all_metrics_met: boolean;
  components: Array<{
    charge: Fx;
    margin: Fx;
    met: boolean;
    minimum_q: Fx;
    node: number;
    present: boolean;
  }>;
  failure_grace_remaining: number;
  failure_streak: number;
  hands_off: boolean;
  hands_off_remaining: number;
  hands_off_streak: number;
  leakage: {
    ceiling: Frac;
    leakage: Fx;
    met: boolean;
    ratio: Frac | null;
    supply: Fx;
  };
  observed_steps: number;
  ready: boolean;
  routes: Array<{
    floor: Fx;
    mean: Fx;
    minimum: Fx;
    met: boolean;
    route: number;
    total: Fx;
    window_steps: number;
  }>;
  status: CriterionStatus;
  step: number;
}

/**
 * The tagged union of causal proposed changes, exactly four variants; `op` is
 * the tag.
 *
 * Every variant costs one Intervention and passive View changes never enter
 * this union. What
 * each is validated against is the core's — the preconditions are locked per
 * variant and checked against the base state with every earlier queued entry
 * applied — so nothing here validates anything: this is the shape the body
 * carries, one to one with the Rust union.
 */
export type PlanCommand =
  | { op: 'connect'; from: number; to: number }
  | { op: 'redirect'; route: number; end: 'tail' | 'head'; to: number }
  | { op: 'cut'; route: number }
  | { op: 'reshape_compartment'; members: number[] }
  | { op: 'deploy_junction' }
  | { op: 'limit_route'; route: number; retained_fraction: number; duration: number }
  | { op: 'raise_leak'; delta: number; duration: number }
  | { op: 'divert_supply'; current: number; receiver: number; capture_fraction: number; duration: number }
  | { op: 'replace_component'; node: number; transfer_mask: number }
  | { op: 'transplant'; regime: RegimeId }
  | { op: 'delay_supply'; current: number; duration: number }
  | { op: 'scramble_routes'; routes: number[]; probability: number; duration: number };

/**
 * The body of the immediate, passive `set_focus` command. Position 0 clears
 * only `view.inside`; positive positions are 1-based candidate seats.
 */
export interface SetFocusBody extends Payload {
  slate_ordinal: number;
  position: number;
}

/** The success body of `set_focus`: the View now used for measurement. */
export interface FocusSet extends Payload {
  view: ViewDeclaration;
}

/** One entry of the queue, as every queued-change response carries it. */
export interface QueueEntry {
  /** Where it stands in the queue, from 0. */
  position: number;
  plan: PlanCommand;
  /** What this causal entry costs. One Intervention, per the locked table. */
  cost: number;
  /**
   * Whether another entry of the queue touches the same Route or proposes the
   * same endpoint pair. A conflict informs the display and invalidates nothing
   * by itself.
   */
  conflict: boolean;
}

/**
 * The queue of proposed changes: what stands in it, what it costs, and the
 * Intervention Budget standing before and after it.
 *
 * `impulse_after` is the prediction the tray shows, and a commit spends exactly
 * `cost_total` — one number arrived at once, so what is displayed and what is
 * spent cannot drift.
 */
export interface QueueState {
  entries: QueueEntry[];
  cost_total: number;
  impulse: number;
  impulse_after: number;
}

/** The success body of `queue_plan`. */
export interface PlanQueued extends Payload {
  queue: QueueState;
}

/** The success body of `undo_plan`: the queue, and how many entries remain. */
export interface PlanUndone extends Payload {
  queue: QueueState;
  remaining: number;
}

/** The success body of `commit_plan`. */
export interface PlanCommitted extends Payload, RunLineage {
  applied: number;
  generator_spec_hash: string;
  impulse: number;
  local_policy: FrozenLocalPolicy;
  route_defaults: RouteControlDefault[];
  scenario_hash: string;
  slate_ordinal: number;
}

/** The request body of `init_run`, in its two modes. */
export type InitRunBody =
  | { mode: 'new'; run_id: string; form: FormId }
  | { mode: 'restore'; save_key: string };

/** The success body of `init_run`, in either mode. */
export interface RunOpened extends Payload, RunLineage {
  protocol: number;
  save_version: number;
  run_id: string;
  step: number;
  chapter_index: number;
  view: ViewDeclaration;
  content_hash: string;
  contract_id: string | null;
  embodied_state_hash: string;
  generator_spec_hash: string;
  local_policy: FrozenLocalPolicy;
  qualification_request: CanonicalQualificationRequest | null;
  qualification_request_id: string | null;
  route_defaults: RouteControlDefault[];
  scenario_hash: string;
  content_changed: boolean;
  regime: RegimeId;
  /** Present only when a V1 record was migrated in memory while opening. */
  migrated_from?: 1 | 2 | 3 | 4 | 5;
}

/** The success body both restores answer with. */
export interface RunRestored extends Payload, RunLineage {
  step: number;
  contract_id: string | null;
  embodied_state_hash: string;
  generator_spec_hash: string;
  local_policy: FrozenLocalPolicy;
  qualification_request: CanonicalQualificationRequest | null;
  qualification_request_id: string | null;
  route_defaults: RouteControlDefault[];
  scenario_hash: string;
  view: ViewDeclaration;
  /** Present only when a V1 record was migrated in memory while restoring. */
  migrated_from?: 1 | 2 | 3 | 4 | 5;
}

/** The success body of `export_run`. */
export interface RunExported extends Payload {
  embodied_state_hash: string;
  text: string;
  sha256: string;
  filename_hint: string;
}

/** The success body of `import_run`. */
export interface RunImported extends Payload, RunLineage {
  run_id: string;
  step: number;
  contract_id: string | null;
  embodied_state_hash: string;
  generator_spec_hash: string;
  local_policy: FrozenLocalPolicy;
  qualification_request: CanonicalQualificationRequest | null;
  qualification_request_id: string | null;
  route_defaults: RouteControlDefault[];
  scenario_hash: string;
  view: ViewDeclaration;
  /** Present only when an older payload was migrated in memory. */
  migrated_from?: 1 | 2 | 3 | 4 | 5;
}

/**
 * One frame of input, one per rendered frame, shell to worker.
 *
 * `seq` strictly increases from 1. `t_us` is the shell's animation-frame
 * timestamp in whole microseconds, and is the only wall clock the simulation
 * ever sees. Steering is normalized to Q1.15, the same shape from pointer,
 * trackpad, and keyboard alike. `advance_steps` replaces the accumulator with
 * exactly that many steps, which is how a deterministic test asks for a step
 * count inside the closed command set.
 */
export interface InputFrame extends Payload {
  seq: number;
  t_us: number;
  steer_x: number;
  steer_y: number;
  pulse_held: boolean;
  pulse_release: boolean;
  wheel: number;
  depth_key: number;
  toggle_still: boolean;
  pause: boolean;
  inspect: InspectRequest | null;
  advance_steps: number | null;
  /** Worker-local wall-time multiplier. Removed before the frame enters Rust. */
  runtime_rate?: 1 | 4 | 16;
}

/** The eight perturbation kinds, by the machine name the core reads. */
export type PerturbationKind =
  | 'boundary-severance'
  | 'route-removal'
  | 'component-substitution'
  | 'resolution-change'
  | 'window-change'
  | 'surround-change'
  | 'delayed-replay'
  | 'full-turnover';

export const PERTURBATION_KINDS: readonly PerturbationKind[] = [
  'boundary-severance',
  'route-removal',
  'component-substitution',
  'resolution-change',
  'window-change',
  'surround-change',
  'delayed-replay',
  'full-turnover',
];

/**
 * One optional inspection a frame asks for.
 *
 * `target` names what to read: the eight recorded-window coordinates, the whole
 * profile with the two the replays pay for, or one perturbation. `kind` is the
 * perturbation's own machine name and null for every other target; `parameter`
 * is the kind's parameter, or null for the kind's own resolved default. The
 * core answers one only while the run is `still` and ignores it otherwise,
 * which is what keeps ordinary play free of readings nobody asked for.
 *
 * `handoff` is the one target that changes the run rather than reading it: it
 * moves control to the Form `parameter` names, immediately and for no Impulse,
 * with `kind` null. ARCHITECTURE.md's `The Handoff` puts it here rather than in
 * the plan queue — the command set is closed and the queue's four causal
 * variants are SPEC-closed, and a Handoff changes no Route, physical
 * Compartment, or View. It
 * refuses on the `input_frame` error path: `not_found` for a Form the run does
 * not hold, `validation` for one the chapter marks un-controllable and for a
 * Handoff to the Form already controlled.
 */
export interface InspectRequest extends Payload {
  target: 'coordinates' | 'coordinates_full' | 'perturbation' | 'handoff';
  kind: PerturbationKind | null;
  parameter: number | null;
}

/**
 * A frame holding nothing down. Every declared field is present, because the
 * protocol has no absent-versus-null ambiguity.
 */
export function neutralFrame(seq: number, timestampUs: number): InputFrame {
  return {
    seq,
    t_us: timestampUs,
    steer_x: 0,
    steer_y: 0,
    pulse_held: false,
    pulse_release: false,
    wheel: 0,
    depth_key: 0,
    toggle_still: false,
    pause: false,
    inspect: null,
    advance_steps: null,
    runtime_rate: 1,
  };
}

/** The body of a `frame` event. `buffer` is the render snapshot, transferred. */
export interface FrameEventBody extends Payload {
  seq: number;
  steps_run: number;
  remainder_us: number;
  dropped: boolean;
  buffer?: ArrayBuffer;
}

/**
 * The one error shape used everywhere. `message_key` is a copy-catalog key for
 * a fault a player is shown and null for a developer-only one; `detail` is
 * machine-readable and is never shown to a player.
 */
export interface ErrorEnvelope {
  code: ErrorCode;
  message_key: string | null;
  detail: Payload | null;
}

/** Shell to worker. `id` is a per-worker-session counter, strictly increasing. */
export interface CommandEnvelope {
  v: number;
  id: number;
  cmd: CommandName;
  body: Payload;
}

/** Worker to shell, exactly one per command; `re` echoes the command's `id`. */
export type ResponseEnvelope =
  | { v: number; re: number; ok: true; body: Payload }
  | { v: number; re: number; ok: false; error: ErrorEnvelope };

/** Worker to shell, unsolicited. */
export interface EventEnvelope {
  v: number;
  ev: EventName;
  step: number;
  body: Payload;
}

/** The thirty-two commands as a value, for membership tests. */
export const COMMAND_NAMES: readonly CommandName[] = [
  'list_contracts',
  'open_contract',
  'init_run',
  'input_frame',
  'queue_plan',
  'undo_plan',
  'commit_plan',
  'set_focus',
  'restore_checkpoint',
  'recover_branch',
  'export_run',
  'import_run',
  'reopen_archive',
  'run_analysis',
  'sample_instrument',
  'inspect_field',
  'compile_scenario',
  'run_scenario',
  'sample_lens',
  'renewal_trial',
  'renewal_inventory',
  'preview_design_patch',
  'commit_design_patch',
  'preview_commission_restart',
  'preview_qualification_input',
  'freeze_qualification_request',
  'qualification_job',
  'engineering_memory',
  'restart_commission',
  'return_commission',
  'resume_commission',
  'set_local_policy',
];

/** True when a value is one of the thirty-two commands. */
export function isCommandName(value: unknown): value is CommandName {
  return typeof value === 'string' && (COMMAND_NAMES as readonly string[]).includes(value);
}

/** The ten events as a value, for membership tests. */
export const EVENT_NAMES: readonly EventName[] = [
  'frame',
  'mechanism_event',
  'criterion_changed',
  'objective_changed',
  'pressure_changed',
  'review_ready',
  'checkpoint_written',
  'chapter_changed',
  'run_completed',
  'qualification_progress',
];

/** True when a value is one of the ten events. */
export function isEventName(value: unknown): value is EventName {
  return typeof value === 'string' && (EVENT_NAMES as readonly string[]).includes(value);
}

/** The body of an `objective_changed` event. */
export interface ObjectiveChanged extends Payload {
  objective: ObjectiveState;
  previous_id: string | null;
}

export interface CriterionChanged extends Payload {
  criterion: CriterionReading | null;
}

/** The body of a `checkpoint_written` event. */
export interface CheckpointWritten extends Payload {
  anchor: CheckpointState;
}

/**
 * Where one candidate of a slate came from, and the detail that source
 * records: the drawn entry's position, the cluster's total weight, the
 * authored position, the total co-response count — and none for the standing
 * View and the three variants.
 */
export interface Provenance extends Payload {
  source:
    | 'standing'
    | 'finer'
    | 'coarser'
    | 'lateral'
    | 'drawn'
    | 'clusters'
    | 'authored'
    | 'responses';
  detail: number | null;
}

/**
 * One of the four privilege values a candidate carries.
 *
 * `value` is the number and `low`/`high` the confidence range that contains
 * it, all as raw `Frac` — the quantity times 65536. An unassigned value
 * carries no number and no range at all, and `reason` says why. The four are
 * never summed, averaged, weighted, or otherwise combined, here or anywhere a
 * reader of this type takes them.
 */
export interface PrivilegeValue extends Payload {
  value: number | null;
  low: number | null;
  high: number | null;
  samples: number;
  reason: string | null;
}

/** The four values, each standing on its own. */
export interface PrivilegeProfile extends Payload {
  scale_stability: PrivilegeValue;
  shared_failure: PrivilegeValue;
  cut_impact: PrivilegeValue;
  boundary_sufficiency: PrivilegeValue;
}

/**
 * One candidate of the standing slate.
 *
 * `position` is the assembly position, 1-based, and presentation order is
 * assembly order. `tier` is the rank the nondominance ranking gives it — 1 for
 * the nondominated set, r + 1 for the set that stands once tiers 1 through r
 * are removed — and 0 on a deficient slate, which is not compared at all.
 */
export interface SlateCandidate extends Payload {
  position: number;
  view: ViewDeclaration;
  provenance: Provenance[];
  tier: number;
  privilege: PrivilegeProfile;
  baseline: { deviations: (number | null)[] };
}

/**
 * The evaluation record, as the core serializes it. The shell reads the
 * ordinal a `set_focus` names, whether it is deficient, the candidates in
 * presentation order with their provenance, four values, and tier, and the
 * tolerance-sensitivity flag — and passes the rest through untouched.
 */
export interface CandidateSlate extends Payload {
  ordinal: number;
  step: number;
  candidates: SlateCandidate[];
  deficient: boolean;
  deficiency_reason: string | null;
  /** The declared window, and the one the clamp left of it. */
  window_declared: number;
  window_effective: number;
  /** Every ordered pair of positions where the first dominates the second. */
  dominance: { a: number; b: number }[];
  /** Whether either tolerance recomputation changed the nondominated set. */
  sensitivity: { flag: boolean; changed_at: string[] };
}

/**
 * One coordinate reading: a number, or no number at all and the stated reason.
 *
 * The unit is the coordinate's own, exactly as FRAMEWORK.md declares it — a
 * count, a raw `Frac`, or a raw `Fx` in distance units — and a reader never
 * combines one with another.
 */
export interface CoordinateReading extends Payload {
  value: number | null;
  reason: string | null;
}

/**
 * The ten-coordinate profile of one View at one step.
 *
 * **No coordinate is combined** with any other, with a privilege value, or with
 * anything else into a single overall value: the ten are reported separately or
 * not at all, here and in every reader of this type. `instruction_separation`
 * and `turnover_tolerance` are the two the replays pay for, and they stand null
 * until a `coordinates_full` request runs them.
 */
export interface CoordinateProfile extends Payload {
  view: ViewDeclaration;
  step: number;
  swap_range: CoordinateReading;
  self_support: CoordinateReading;
  throughput: {
    in_rate: number;
    out_rate: number;
    routes: { route: number; rate: number }[];
    shell: { node: number; rate: number }[];
  };
  upkeep_mix: { value: number[] | null; reason: string | null };
  reach: CoordinateReading;
  input_resolution: CoordinateReading;
  horizon: CoordinateReading;
  source_trace: CoordinateReading;
  instruction_separation: PrivilegeValue | null;
  turnover_tolerance: {
    value: number | null;
    reason: string | null;
    pairs: { phi: number; agreement: number }[] | null;
  } | null;
}

/**
 * One replayed sample of a perturbation, with the compact playback record it
 * retained.
 *
 * `series` is the replayed inside's stored-Charge series, one raw `Fx` per
 * replayed step. `base_series` and `base_deviation` stand beside it only for
 * `delayed-replay`, whose base is its own unshifted-schedule replay; for every
 * other kind they are null and the excess is taken against the shared baseline
 * of the same sample number.
 */
export interface PerturbationSample extends Payload {
  deviation: number | null;
  excess: number | null;
  series: number[];
  base_deviation: number | null;
  base_series: number[] | null;
}

/**
 * One perturbation result, as the core records it.
 *
 * `parameter` always carries the value the kind actually used — a defaulted
 * parameter is stored resolved — and is null only for the two kinds that take
 * none. Results are session-lived: they never enter a save payload, and what
 * reproduces one after a restore is `sigma` with the resolved parameter.
 */
export interface PerturbationResult extends Payload {
  view: ViewDeclaration;
  provenance: Provenance[];
  position: number;
  sigma: Payload;
  streams: string[];
  kind: PerturbationKind;
  parameter: number | null;
  tau: number;
  reading: PrivilegeValue;
  samples: PerturbationSample[];
  recomputed: Payload | null;
  step: number;
}

/**
 * The one short causal highlight a committed change leaves.
 *
 * `kind` is a perturbation machine name, or `evaluation` when the highlight
 * derives from the adopted candidate's evaluation record. The three values are
 * the largest excess deviation and its confidence range; the shell chooses the
 * catalog wording and shows no number at all.
 */
export interface EchoHighlight extends Payload {
  kind: PerturbationKind | 'evaluation';
  parameter: number | null;
  excess: number;
  low: number;
  high: number;
  target: { t: 'node' | 'route' | 'none'; id: number | null };
}

/** One compact authoritative transition carried into the diagnostic timeline. */
export type MechanismEvent =
  | {
      kind: 'policy';
      action: LocalAction['kind'] | 'none';
      address: number;
      object_id: number;
      object_kind: 'form' | 'node';
      outcome: PolicyOutcome;
      rule: number;
      target: number | null;
      target_kind: PolicyTargetKind;
    }
  | { kind: 'interface'; node: number; open: boolean }
  | {
      kind: 'route';
      route: number;
      enabled: boolean;
      capacity_limit: Fx;
      allocation_weight: number;
      requested_flow: Fx;
      accepted_flow: Fx;
      state: 'disabled' | 'closed' | 'standing' | 'capacity_throttled' | 'source_starved' | 'destination_headroom' | 'flowing';
    }
  | { kind: 'supply'; current: number; emitting: boolean }
  | {
      kind: 'reserve';
      form: number;
      node: number;
      opening: Fx;
      closing: Fx;
      delta: Fx;
      state: 'banked' | 'released';
    }
  | {
      kind: 'charge';
      accepted_supply: Fx;
      closing: Fx;
      coupled_transfer: Fx;
      drain: Fx;
      dominant_node: number | null;
      leakage: Fx;
      nodes: Array<{
        closing: Fx;
        exogenous: Fx;
        inflow: Fx;
        leakage: Fx;
        node: number;
        opening: Fx;
        outflow: Fx;
        supply: Fx;
        upkeep: Fx;
      }>;
      opening: Fx;
      route_transfer: Fx;
      upkeep: Fx;
    }
  | { kind: 'criterion'; chapter_index: number; status: CriterionStatus }
  | { kind: 'failure'; chapter_index: number; source: 'criterion' };

/** The body of a `review_ready` event: one of the four reviews. */
export interface ReviewReady extends Payload {
  review:
    | { kind: 'slate'; slate: CandidateSlate }
    | { kind: 'echo'; echo: EchoHighlight }
    | { kind: 'coordinates'; profile: CoordinateProfile }
    | { kind: 'perturbation'; result: PerturbationResult }
    | { kind: string };
}

/** The body of a `chapter_changed` event. */
export interface ChapterChanged extends Payload {
  chapter_count: number;
  chapter_index: number;
  objective_count: number;
  route_defaults: RouteControlDefault[];
  title_key: string;
  /** The complete authoritative passive View at this chapter boundary. */
  view: ViewDeclaration;
}

/**
 * The body of a `run_completed` event: the ending the campaign closed on.
 *
 * `ending_id` is the copy-catalog key the last chapter authored, so the shell
 * reads the wording from the catalog by the key the worker names and never
 * chooses one. `continuation_unlocked` is true exactly when the campaign the
 * run completed authored the whole closed set of chapters.
 */
export interface RunCompleted extends Payload {
  ending_id: string;
  chapter_index: number;
  continuation_unlocked: boolean;
}

/** True when a value is a `u32`, the width of `id` and `re`. */
export function isCorrelationId(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0 && value <= 0xffffffff;
}

/** Builds the error response for a command that could not be served. */
export function errorResponse(re: number, error: ErrorEnvelope): ResponseEnvelope {
  return { v: PROTOCOL_VERSION, re, ok: false, error };
}
