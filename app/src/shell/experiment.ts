import type { FrameState } from '../../../worker/src/frame-state';
import type { CriterionReading, FormId, Surround, ViewDeclaration } from '../../../worker/src/protocol';
import type { RegimeId } from './Atlas';

export type BenchId =
  | 'observe'
  | 'intervene'
  | 'divergence'
  | 'ensemble'
  | 'holdout'
  | 'archive'
  | 'renewal'
  | 'inheritance'
  | 'open_field';

export type AnalysisTask = 'divergence' | 'ensemble' | 'holdout' | 'inheritance';

export type InstrumentId =
  | 'stored_charge'
  | 'route_flow'
  | 'view_boundary_flow'
  | 'supply_uptake'
  | 'physical_leakage'
  | 'maintenance_allocation'
  | 'initial_stock_estimate'
  | 'response_lag';

export type InterventionId =
  | 'blade'
  | 'clamp'
  | 'scramble'
  | 'decoy'
  | 'delay'
  | 'replace'
  | 'breach'
  | 'transplant';

export interface ObservationProtocol {
  instrument: InstrumentId;
  resolution: number;
  window: number;
  surround: Surround;
}

export interface InstrumentReading {
  agreement: number;
  effectiveWindow: number;
  instrument: InstrumentId;
  primary: number;
  provenance: string;
  secondary: number;
  samples: number[];
  step: number;
  targetCount: number;
}

export interface InterventionPlan {
  id: string;
  tool: InterventionId;
  scope: 'replay' | 'live';
  target: number;
  receiver: number;
  transferMask: number;
  destination: RegimeId;
  onset: number;
  duration: number;
  amount: number;
}

export interface AnalysisScenario {
  id: string;
  regime: RegimeId;
  form: FormId;
  step: number;
  chapter: number;
  control: 'recorded_open_loop' | 'hands_off';
  controlContract: {
    steering: boolean;
    pulse: boolean;
    handoff: boolean;
    rescue: boolean;
    record: 'frame_sequence' | 'none';
  };
  view: ViewDeclaration | null;
  observation: ObservationProtocol;
  intervention: InterventionPlan | null;
  routeIds: number[];
  nodeIds: number[];
  holdoutSeed?: number;
}

export interface TracePoint {
  step: number;
  baseline: number;
  changed: number;
}

export interface LensForecastPoint {
  step: number;
  low: number;
  expected: number;
  high: number;
}

export interface LensSensorPacket {
  cost: number;
  horizon: number;
  node_ids: number[];
  points: LensForecastPoint[];
  route_ids: number[];
  sensor_radius: number;
}

export interface DivergenceResult {
  controlContract: AnalysisScenario['control'];
  generatorHash: string;
  scenarioHash: string;
  firstStep: number;
  points: TracePoint[];
  outletFloorStep: number;
  reserveFloorStep: number;
  criterionStep: number;
}

export interface EnsembleTrial {
  criterion: CriterionReading | null;
  criterionStatus: 'active' | 'failed' | 'passed' | 'unavailable';
  seed: number;
  value: number;
  passed: boolean;
  failure: 'none' | 'throughput' | 'reserve' | 'leakage';
  trace: number[];
}

export interface EnsembleResult {
  controlContract: AnalysisScenario['control'];
  generatorHash: string;
  scenarioHash: string;
  trials: EnsembleTrial[];
  median: number;
  low: number;
  high: number;
  passCount: number;
}

export interface RenewalResult {
  controlContract: AnalysisScenario['control'];
  embodiedStateHash: string;
  generatorHash: string;
  scenarioHash: string;
  seed: number;
  detectedAt: number;
  recruitedAt: number;
  reconnectedAt: number;
  recoveredAt: number;
  resourceCost: number;
  materialCost: number;
  materialIds: number[];
  rebuiltRoutes: number[];
  signalId: number | null;
  reconnection: number;
  failedNode: number;
  replacementNode: number | null;
  passed: boolean;
}

export interface RenewalInventory {
  step: number;
  materials: Array<{
    amount: number;
    claimed: boolean;
    kind: 'junction_blank' | 'boundary_blank' | 'conductor';
    layer: number;
    material: number;
    x: number;
    y: number;
  }>;
  signals: Array<{
    emitted_step: number;
    expires_step: number;
    layer: number;
    signal: number;
    source: number;
    strength: number;
    target: number;
    x: number;
    y: number;
  }>;
}

export interface InheritanceChild {
  id: 'A' | 'B';
  inheritedComponents: number;
  inheritedRoutes: number;
  componentIds: number[];
  routeIds: number[];
  initialCharge: number;
  recoveredAt: number | null;
  criterionMargin: number;
  passed: boolean;
}

export interface InheritanceResult {
  controlContract: AnalysisScenario['control'];
  generatorHash: string;
  scenarioHash: string;
  copiedSpecification: string;
  partition: 'alternating_components';
  sourceComponents: number;
  sourceRoutes: number;
  children: [InheritanceChild, InheritanceChild];
}

export type AnalysisResult =
  | DivergenceResult
  | EnsembleResult
  | RenewalResult[]
  | InheritanceResult;

export const DEFAULT_OBSERVATION: ObservationProtocol = {
  instrument: 'stored_charge',
  resolution: 8,
  window: 45,
  surround: 'adjacent',
};

export const BENCHES: readonly BenchId[] = [
  'observe',
  'intervene',
  'divergence',
  'ensemble',
  'holdout',
  'archive',
  'renewal',
  'inheritance',
  'open_field',
];

export const INSTRUMENTS: readonly InstrumentId[] = [
  'stored_charge',
  'route_flow',
  'view_boundary_flow',
  'supply_uptake',
  'physical_leakage',
  'maintenance_allocation',
  'initial_stock_estimate',
  'response_lag',
];

export const INTERVENTIONS: readonly InterventionId[] = [
  'blade',
  'clamp',
  'scramble',
  'decoy',
  'delay',
  'replace',
  'breach',
  'transplant',
];

/** Tools whose typed transition currently reaches the authoritative live queue. */
export const LIVE_INTERVENTIONS: readonly InterventionId[] = ['blade', 'clamp', 'scramble', 'decoy', 'delay', 'replace', 'breach', 'transplant'];

export function scenarioFrom(
  frame: FrameState,
  regime: RegimeId,
  form: FormId,
  view: ViewDeclaration | null,
  observation: ObservationProtocol,
  intervention: InterventionPlan | null,
  control: AnalysisScenario['control'] = 'recorded_open_loop',
): AnalysisScenario {
  const identity = [
    regime,
    form,
    frame.header.step,
    frame.header.chapterIndex,
    observation.instrument,
    observation.resolution,
    observation.window,
    observation.surround,
    intervention?.tool ?? 'neutral',
    intervention?.target ?? 0,
    intervention?.receiver ?? 0,
    intervention?.transferMask ?? 0,
    intervention?.destination ?? 'open_field',
    control,
  ].join(':');
  let hash = 2_166_136_261;
  for (let place = 0; place < identity.length; place += 1) {
    hash ^= identity.charCodeAt(place);
    hash = Math.imul(hash, 16_777_619);
  }
  return {
    id: (hash >>> 0).toString(16).padStart(8, '0'),
    regime,
    form,
    step: frame.header.step,
    chapter: frame.header.chapterIndex,
    control,
    controlContract: control === 'hands_off'
      ? { steering: false, pulse: false, handoff: false, rescue: false, record: 'none' }
      : { steering: true, pulse: true, handoff: true, rescue: true, record: 'frame_sequence' },
    view,
    observation,
    intervention,
    routeIds: frame.routes.map((route) => route.route),
    nodeIds: frame.ports.map((port) => port.node),
  };
}
