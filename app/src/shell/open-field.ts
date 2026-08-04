import type { CriterionReading, FormId } from '../../../worker/src/protocol';
import type { CoreClient } from './worker-client';

export type OpenFieldLawsetId =
  | 'discrete-transport-v1'
  | 'discrete-transport-crowded-v1'
  | 'discrete-transport-vestige-v1'
  | 'discrete-transport-holdout-v1';

export const OPEN_FIELD_LAWSETS: readonly OpenFieldLawsetId[] = [
  'discrete-transport-v1',
  'discrete-transport-crowded-v1',
  'discrete-transport-vestige-v1',
  'discrete-transport-holdout-v1',
];

const LAWSET_DEFAULTS: Record<OpenFieldLawsetId, Pick<OpenFieldDraft,
  'supplyPerStep' | 'supplyWidth' | 'dissipationPerStep' | 'conductanceNoise' | 'routeCapacityScale' | 'compartmentLeak'>> = {
  'discrete-transport-v1': { supplyPerStep: 2, supplyWidth: 64, dissipationPerStep: 0, conductanceNoise: 0.125, routeCapacityScale: 1, compartmentLeak: 0.015625 },
  'discrete-transport-crowded-v1': { supplyPerStep: 1.5, supplyWidth: 48, dissipationPerStep: 0.0625, conductanceNoise: 0.25, routeCapacityScale: 0.75, compartmentLeak: 0.03125 },
  'discrete-transport-vestige-v1': { supplyPerStep: 1, supplyWidth: 32, dissipationPerStep: 0.125, conductanceNoise: 0.375, routeCapacityScale: 0.625, compartmentLeak: 0.125 },
  'discrete-transport-holdout-v1': { supplyPerStep: 2, supplyWidth: 64, dissipationPerStep: 0.03125, conductanceNoise: 0.1875, routeCapacityScale: 1, compartmentLeak: 0.015625 },
};

export interface OpenFieldDraft {
  lawsetId: OpenFieldLawsetId;
  form: FormId;
  supplyPerStep: number;
  supplyWidth: number;
  dissipationPerStep: number;
  conductanceNoise: number;
  routeCapacityScale: number;
  compartmentLeak: number;
  observationWindow: number;
  observationResolution: number;
  criterionFloor: number;
  criterionRouteFloor: number;
  criterionLeakageCeiling: number;
  criterionWindow: number;
  criterionFailureGrace: number;
  criterionDuration: number;
  trialCount: number;
  control: 'recorded_open_loop' | 'hands_off';
  intervention: OpenFieldInterventionDraft;
  components: OpenFieldComponentDraft[];
  materials: OpenFieldMaterialDraft[];
  routes: OpenFieldRouteDraft[];
  compartmentMembers: number[];
  supplyLayer: number;
  supplyX: number;
  supplyY: number;
}

export function withLawset(draft: OpenFieldDraft, lawsetId: OpenFieldLawsetId): OpenFieldDraft {
  return { ...draft, lawsetId, ...LAWSET_DEFAULTS[lawsetId] };
}

export interface OpenFieldComponentDraft {
  node: number;
  kind: 'port' | 'reserve' | 'module';
  layer: number;
  x: number;
  y: number;
  charge: number;
  open: boolean;
  upkeepRate: number;
  capacity: number;
}

export interface OpenFieldRouteDraft {
  route: number;
  tail: number;
  head: number;
  capacity: number;
}

export interface OpenFieldMaterialDraft {
  material: number;
  kind: 'junction_blank' | 'boundary_blank' | 'conductor';
  amount: number;
  layer: number;
  x: number;
  y: number;
}

export interface OpenFieldInterventionDraft {
  tool: 'none' | 'blade' | 'clamp' | 'breach';
  target: number;
  onset: number;
  duration: number;
  amount: number;
}

export interface CompiledOpenField {
  experimentId: string;
  scenarioHash: string;
  canonical: string;
  draft: OpenFieldDraft;
}

export interface OpenFieldTrial {
  criterion: CriterionReading;
  final_charge: number;
  minimum_selected_charge: number;
  passed: boolean;
  samples: number;
  seed: number;
  sustained_steps: number;
}

export interface OpenFieldRun {
  controlContract: 'hands_off';
  embodiedStateHash: string;
  experimentId: string;
  generatorHash: string;
  passed: number;
  scenarioHash: string;
  trials: OpenFieldTrial[];
}

export const DEFAULT_OPEN_FIELD: OpenFieldDraft = {
  lawsetId: 'discrete-transport-v1',
  form: 'thread',
  supplyPerStep: 2,
  supplyWidth: 64,
  dissipationPerStep: 0,
  conductanceNoise: 0.125,
  routeCapacityScale: 1,
  compartmentLeak: 0.015625,
  observationWindow: 180,
  observationResolution: 8,
  criterionFloor: 8,
  criterionRouteFloor: 1,
  criterionLeakageCeiling: 0.15,
  criterionWindow: 30,
  criterionFailureGrace: 8,
  criterionDuration: 60,
  trialCount: 8,
  control: 'hands_off',
  intervention: { tool: 'none', target: 0, onset: 12, duration: 90, amount: 35 },
  components: [],
  materials: [],
  routes: [],
  compartmentMembers: [],
  supplyLayer: 0,
  supplyX: 2048,
  supplyY: 2048,
};

const fixed = (value: number): number => Math.round(value * 65_536);

function draftPayload(draft: OpenFieldDraft) {
  return {
    compartment_leak: fixed(draft.compartmentLeak),
    conductance_noise: fixed(draft.conductanceNoise),
    control: draft.control,
    components: draft.components.map((component) => ({
      capacity: fixed(component.capacity),
      charge: fixed(component.charge),
      kind: component.kind,
      layer: Math.round(component.layer),
      node: Math.round(component.node),
      open: component.open,
      upkeep_rate: fixed(component.upkeepRate),
      x: fixed(component.x),
      y: fixed(component.y),
    })),
    compartment_members: [...draft.compartmentMembers].sort((left, right) => left - right),
    criterion_duration: Math.round(draft.criterionDuration),
    criterion_failure_grace: Math.round(draft.criterionFailureGrace),
    criterion_floor: fixed(draft.criterionFloor),
    criterion_leakage_ceiling: fixed(draft.criterionLeakageCeiling),
    criterion_route_floor: fixed(draft.criterionRouteFloor),
    criterion_window: Math.round(draft.criterionWindow),
    dissipation_per_step: fixed(draft.dissipationPerStep),
    form: draft.form,
    intervention: {
      amount: Math.round(draft.intervention.amount),
      duration: Math.round(draft.intervention.duration),
      onset: Math.round(draft.intervention.onset),
      target: Math.round(draft.intervention.target),
      tool: draft.intervention.tool,
    },
    lawset_id: draft.lawsetId,
    materials: draft.materials.map((material) => ({
      amount: Math.round(material.amount),
      kind: material.kind,
      layer: Math.round(material.layer),
      material: Math.round(material.material),
      x: fixed(material.x),
      y: fixed(material.y),
    })),
    observation_resolution: Math.round(draft.observationResolution),
    observation_window: Math.round(draft.observationWindow),
    route_capacity_scale: fixed(draft.routeCapacityScale),
    routes: draft.routes.map((route) => ({
      capacity: fixed(route.capacity),
      head: Math.round(route.head),
      route: Math.round(route.route),
      tail: Math.round(route.tail),
    })),
    supply_per_step: fixed(draft.supplyPerStep),
    supply_width: fixed(draft.supplyWidth),
    supply_layer: Math.round(draft.supplyLayer),
    supply_x: fixed(draft.supplyX),
    supply_y: fixed(draft.supplyY),
    trial_count: Math.round(draft.trialCount),
  };
}

export async function compileOpenField(
  draft: OpenFieldDraft,
  client: CoreClient,
): Promise<CompiledOpenField> {
  const answer = await client.command('compile_scenario', {
    draft: draftPayload(draft),
  });
  if (!answer.ok) throw new Error(answer.error.message_key ?? answer.error.code);
  const body = answer.body as {
    canonical?: string;
    experiment_id?: string;
    scenario_hash?: string;
  };
  if (!body.canonical || !body.experiment_id || !body.scenario_hash) {
    throw new Error('compiled_scenario_incomplete');
  }
  return {
    experimentId: body.experiment_id,
    scenarioHash: body.scenario_hash,
    canonical: body.canonical,
    draft,
  };
}

export async function runOpenField(
  draft: OpenFieldDraft,
  client: CoreClient,
): Promise<OpenFieldRun> {
  const answer = await client.command('run_scenario', { draft: draftPayload(draft) });
  if (!answer.ok) throw new Error(answer.error.message_key ?? answer.error.code);
  const body = answer.body as unknown as {
    control_contract: 'hands_off';
    embodied_state_hash: string;
    experiment_id: string;
    generator_hash: string;
    passed: number;
    scenario_hash: string;
    trials: OpenFieldTrial[];
  };
  return {
    controlContract: body.control_contract,
    embodiedStateHash: body.embodied_state_hash,
    experimentId: body.experiment_id,
    generatorHash: body.generator_hash,
    passed: body.passed,
    scenarioHash: body.scenario_hash,
    trials: body.trials,
  };
}
