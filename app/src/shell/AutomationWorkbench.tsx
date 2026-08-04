import { useCallback, useEffect, useMemo, useState } from 'react';
import type { FrameMode, FrameRoute } from '../../../worker/src/frame-state';
import type {
  ComponentPolicy,
  CommissionRestartPreview,
  ContractCatalogEntry,
  CriterionReading,
  DesignPreviewed,
  FrozenLocalPolicy,
  LocalAction,
  LocalCondition,
  PolicyRule,
  PolicyPreview,
  PressureState,
  QualificationInputPreview,
  QualificationCriterionDecision,
  QualificationFunctionDecision,
  QualificationGrade,
  QualificationFailureTrace,
  QualificationJob,
  QualificationResultGroup,
  QualificationUnlockReceipt,
  EngineeringAssemblyDraft,
  EngineeringAssemblyDiffChange,
  EngineeringAssemblyDraftResult,
  EngineeringAssemblyPreview,
  EngineeringRunTransitionPreview,
  EngineeringTransitionKind,
  EngineeringTransitionPreviewAccepted,
  EngineeringTransitionRefused,
  QualificationTrialArtifact,
  ResponseEnvelope,
  RouteControlDefault,
} from '../../../worker/src/protocol';
import { copy } from './copy';
import type {
  CommissionAttemptRecord,
  EngineeringBlueprintEntry,
  EngineeringGeneratorSourceEntry,
  EngineeringMigrationJournal,
  EngineeringOperationRecovery,
} from './archive';
import type { FieldInspection } from './FieldSurface';
import type {
  CommissionBreakpoint,
  MechanismTimelineEntry,
  QueueState,
  RunIdentity,
} from './worker-client';
import type { StillTool } from './still-edits';
import './automation-workbench.css';

const WHOLE = 65_536;
const MAX_RULES = 8;

type ConditionKind = LocalCondition['kind'];
type ActionKind = LocalAction['kind'];
type RestartUiState = 'closed' | 'previewing' | 'ready' | 'submitting';

const CONDITION_KINDS: readonly ConditionKind[] = [
  'always',
  'charge_below',
  'charge_above',
  'operating_margin_below',
  'supply',
  'target_in_range',
  'route_flow_below',
  'route_flow_above',
  'overloaded',
  'signal_present',
  'timer_elapsed',
];

const MOBILE_ACTIONS: readonly ActionKind[] = [
  'hold',
  'seek_supply',
  'seek_port',
  'seek_signal',
  'change_depth',
  'couple',
  'set_interface',
  'set_route',
  'emit_signal',
  'use_ability',
];

const STATIONARY_ACTIONS: readonly ActionKind[] = [
  'hold',
  'set_interface',
  'set_route',
  'emit_signal',
];

function fixed(value: number): string {
  return (value / WHOLE).toLocaleString('en-US', { maximumFractionDigits: 2 });
}

function rawUnits(value: string, floor = 0, ceiling = Number.MAX_SAFE_INTEGER): number {
  const parsed = Number(value);
  return Number.isFinite(parsed)
    ? Math.min(ceiling, Math.max(floor, Math.round(parsed * WHOLE)))
    : floor;
}

function integer(value: string, floor = 0, ceiling = 65_535): number {
  const parsed = Number(value);
  return Number.isFinite(parsed)
    ? Math.min(ceiling, Math.max(floor, Math.round(parsed)))
    : floor;
}

function firstRoute(routes: readonly FrameRoute[]): FrameRoute | undefined {
  return routes[0];
}

function conditionFrom(kind: ConditionKind, routes: readonly FrameRoute[]): LocalCondition {
  switch (kind) {
    case 'always': return { kind };
    case 'charge_below':
    case 'charge_above': return { kind, fraction: WHOLE / 2 };
    case 'operating_margin_below': return { kind, amount: 2 * WHOLE };
    case 'supply': return { kind, state: 'emitting', radius: 240 * WHOLE };
    case 'target_in_range': return { kind, radius: 72 * WHOLE };
    case 'route_flow_below':
    case 'route_flow_above': return { kind, route: firstRoute(routes)?.route ?? 1, flow: WHOLE };
    case 'overloaded': return { kind };
    case 'signal_present': return { kind, radius: 180 * WHOLE };
    case 'timer_elapsed': return { kind, steps: 30 };
  }
}

function actionFrom(kind: ActionKind, routes: readonly FrameRoute[]): LocalAction {
  switch (kind) {
    case 'hold': return { kind };
    case 'seek_supply':
    case 'seek_port':
    case 'seek_signal': return { kind, radius: 240 * WHOLE };
    case 'couple': return { kind, radius: 72 * WHOLE };
    case 'change_depth': return { kind, direction: 1 };
    case 'set_interface': return { kind, open: true };
    case 'set_route': {
      const route = firstRoute(routes);
      return {
        kind,
        route: route?.route ?? 1,
        enabled: route?.automationEnabled ?? true,
        capacity_limit: route?.capacityLimit ?? route?.capacity ?? 0,
        allocation_weight: route?.allocationWeight ?? 1,
      };
    }
    case 'emit_signal': return { kind, strength: WHOLE };
    case 'use_ability': return { kind };
  }
}

function defaultPolicy(
  address: number,
  mobile: boolean,
  routes: readonly FrameRoute[],
  contractId: string | null,
): ComponentPolicy {
  const rules: PolicyRule[] = mobile && contractId === 'buffer'
    ? [
        {
          enabled: true,
          condition: { kind: 'supply', state: 'emitting', radius: 4_096 * WHOLE },
          action: { kind: 'seek_supply', radius: 4_096 * WHOLE },
        },
        {
          enabled: true,
          condition: { kind: 'target_in_range', radius: 192 * WHOLE },
          action: { kind: 'use_ability' },
        },
      ]
    : mobile
    ? [
        {
          enabled: true,
          condition: { kind: 'target_in_range', radius: 72 * WHOLE },
          action: { kind: 'couple', radius: 72 * WHOLE },
        },
        {
          enabled: true,
          condition: { kind: 'supply', state: 'emitting', radius: 240 * WHOLE },
          action: { kind: 'seek_supply', radius: 240 * WHOLE },
        },
      ]
    : [{
        enabled: true,
        condition: { kind: 'always' },
        action: { kind: 'set_interface', open: true },
      }];
  const fallback = mobile && contractId === 'buffer'
    ? { kind: 'seek_port', radius: 4_096 * WHOLE } as const
    : actionFrom('hold', routes);
  return { address, rules, fallback };
}

function policyWithComponent(
  policy: FrozenLocalPolicy,
  component: ComponentPolicy,
): FrozenLocalPolicy {
  return {
    version: 2,
    components: [
      ...policy.components.filter((held) => held.address !== component.address),
      component,
    ].sort((first, second) => first.address - second.address),
  };
}

function componentAddress(selection: FieldInspection | null): number | null {
  if (!selection) return null;
  if (selection.target === 'form') return selection.node;
  if (selection.target === 'node') return selection.id;
  return null;
}

function selectionName(selection: FieldInspection): string {
  if (selection.target === 'form') return copy(`form.${selection.kind}`);
  if (selection.target === 'node') return copy(`node.kind.${selection.kind}`);
  return copy(`field.inspect.${selection.target}`);
}

function actionLabel(action: LocalAction | null): string {
  return action ? copy(`automation.action.${action.kind}`) : copy('automation.status.no_action');
}

function mechanismLabel(entry: MechanismTimelineEntry): string {
  const event = entry.event;
  switch (event.kind) {
    case 'policy':
      return `${copy('automation.event.policy')} ${event.address}: ${copy(`automation.action.${event.action}`)} · ${copy(`automation.outcome.${event.outcome}`)}`;
    case 'interface':
      return `${copy('automation.event.interface')} ${event.node}: ${copy(event.open ? 'automation.interface.open' : 'automation.interface.closed')}`;
    case 'route':
      return `${copy('automation.label.route')} ${event.route}: ${copy(`automation.route.${event.state}`)} · ${copy('automation.label.requested_flow')} ${fixed(event.requested_flow)} · ${copy('automation.label.accepted_flow')} ${fixed(event.accepted_flow)}`;
    case 'supply':
      return `${copy('automation.event.supply')} ${event.current}: ${copy(event.emitting ? 'automation.supply.emitting' : 'automation.supply.quiet')}`;
    case 'reserve':
      return `${copy('automation.event.reserve')} ${event.form}: ${copy(`automation.reserve.${event.state}`)} · ${fixed(event.opening)} ${copy('automation.label.to')} ${fixed(event.closing)} ${copy('unit.cu')}`;
    case 'charge':
      return `${copy('automation.event.charge')} · ${copy('automation.event.accepted_supply')} ${fixed(event.accepted_supply)} · ${copy('automation.event.coupled')} ${fixed(event.coupled_transfer)} · ${copy('automation.event.routed')} ${fixed(event.route_transfer)} · ${copy('automation.event.upkeep')} ${fixed(event.upkeep)} · ${copy('automation.event.loss')} ${fixed(event.leakage + event.drain)}`;
    case 'criterion':
      return `${copy('automation.criterion.label')} ${copy(`automation.criterion.${event.status}`)}`;
    case 'failure':
      return copy('automation.event.criterion_failure');
  }
}

function mechanismSelectable(entry: MechanismTimelineEntry): boolean {
  return ['policy', 'interface', 'route', 'supply', 'reserve'].includes(entry.event.kind)
    || entry.event.kind === 'charge' && entry.event.dominant_node !== null;
}

function attachedRoutes(address: number, routes: readonly FrameRoute[]): FrameRoute[] {
  return routes.filter((route) => route.tail === address || route.head === address);
}

function outgoingRoutes(address: number, routes: readonly FrameRoute[]): FrameRoute[] {
  return routes.filter((route) => route.tail === address);
}

function embodiedRouteDefaults(routes: readonly FrameRoute[]): RouteControlDefault[] {
  return routes
    .filter((route) => (route.status & 0x0f) !== 4)
    .map((route) => ({
      route: route.route,
      enabled: route.automationEnabled,
      capacity_limit: route.capacityLimit,
      allocation_weight: route.allocationWeight,
      controller: route.controller,
    }))
    .sort((first, second) => first.route - second.route);
}

interface ConditionEditorProps {
  condition: LocalCondition;
  routes: readonly FrameRoute[];
  available: readonly ConditionKind[];
  couplingMaximum: number;
  sensorMaximum: number;
  onChange: (condition: LocalCondition) => void;
}

function ConditionEditor({
  condition,
  routes,
  available,
  couplingMaximum,
  sensorMaximum,
  onChange,
}: ConditionEditorProps) {
  const routeKindsAvailable = routes.length > 0;
  return (
    <div className="automation-clause">
      <label>
        <span>{copy('automation.label.when')}</span>
        <select
          value={condition.kind}
          onChange={(event) => onChange(conditionFrom(event.target.value as ConditionKind, routes))}
        >
          {CONDITION_KINDS.filter((kind) => (
            available.includes(kind) && (routeKindsAvailable || !kind.startsWith('route_flow'))
          ) || kind === condition.kind).map((kind) => (
            <option
              key={kind}
              value={kind}
              disabled={!available.includes(kind) || (!routeKindsAvailable && kind.startsWith('route_flow'))}
            >
              {copy(`automation.condition.${kind}`)}
            </option>
          ))}
        </select>
      </label>
      {condition.kind === 'charge_below' || condition.kind === 'charge_above' ? (
        <label className="automation-parameter">
          <span>{copy('automation.label.capacity_percent')}</span>
          <input
            type="number"
            min="0"
            max="100"
            step="1"
            value={Math.round(condition.fraction * 100 / WHOLE)}
            onChange={(event) => onChange({
              ...condition,
              fraction: integer(event.target.value, 0, 100) * WHOLE / 100,
            })}
          />
        </label>
      ) : null}
      {condition.kind === 'operating_margin_below' ? (
        <UnitInput
          label="automation.label.margin_cu"
          value={condition.amount}
          onChange={(amount) => onChange({ ...condition, amount })}
        />
      ) : null}
      {condition.kind === 'supply' ? (
        <>
          <label className="automation-parameter">
            <span>{copy('automation.label.supply_state')}</span>
            <select
              value={condition.state}
              onChange={(event) => onChange({
                ...condition,
                state: event.target.value as typeof condition.state,
              })}
            >
              {(['absent', 'present', 'emitting', 'quiet'] as const).map((state) => (
                <option key={state} value={state}>{copy(`automation.supply.${state}`)}</option>
              ))}
            </select>
          </label>
          <UnitInput
            label="automation.label.sensor_radius"
            value={condition.radius}
            maximum={sensorMaximum}
            onChange={(radius) => onChange({ ...condition, radius })}
          />
        </>
      ) : null}
      {condition.kind === 'target_in_range' || condition.kind === 'signal_present' ? (
        <UnitInput
          label={condition.kind === 'target_in_range'
            ? 'automation.label.coupling_radius'
            : 'automation.label.sensor_radius'}
          value={condition.radius}
          maximum={condition.kind === 'target_in_range' ? couplingMaximum : sensorMaximum}
          onChange={(radius) => onChange({ ...condition, radius })}
        />
      ) : null}
      {condition.kind === 'route_flow_below' || condition.kind === 'route_flow_above' ? (
        <>
          <RouteSelect
            value={condition.route}
            routes={routes}
            onChange={(route) => onChange({ ...condition, route })}
          />
          <UnitInput
            label="automation.label.flow_cu_step"
            value={condition.flow}
            onChange={(flow) => onChange({ ...condition, flow })}
          />
        </>
      ) : null}
      {condition.kind === 'timer_elapsed' ? (
        <label className="automation-parameter">
          <span>{copy('automation.label.timer_steps')}</span>
          <input
            type="number"
            min="1"
            max="65535"
            step="1"
            value={condition.steps}
            onChange={(event) => onChange({ ...condition, steps: integer(event.target.value, 1) })}
          />
        </label>
      ) : null}
    </div>
  );
}

interface ActionEditorProps {
  action: LocalAction;
  mobile: boolean;
  routes: readonly FrameRoute[];
  available: readonly ActionKind[];
  couplingMaximum: number;
  sensorMaximum: number;
  signalMaximum: number;
  onChange: (action: LocalAction) => void;
}

function ActionEditor({
  action,
  mobile,
  routes,
  available,
  couplingMaximum,
  sensorMaximum,
  signalMaximum,
  onChange,
}: ActionEditorProps) {
  const ordered = mobile ? MOBILE_ACTIONS : STATIONARY_ACTIONS;
  const kinds = ordered.filter((kind) => (
    available.includes(kind) && (kind !== 'set_route' || routes.length > 0)
  ) || kind === action.kind);
  return (
    <div className="automation-clause">
      <label>
        <span>{copy('automation.label.then')}</span>
        <select
          value={action.kind}
          onChange={(event) => onChange(actionFrom(event.target.value as ActionKind, routes))}
        >
          {kinds.map((kind) => (
            <option
              key={kind}
              value={kind}
              disabled={!available.includes(kind) || (kind === 'set_route' && routes.length === 0)}
            >
              {copy(`automation.action.${kind}`)}
            </option>
          ))}
        </select>
      </label>
      {action.kind === 'seek_supply' || action.kind === 'seek_port' || action.kind === 'seek_signal' || action.kind === 'couple' ? (
        <UnitInput
          label={action.kind === 'couple' ? 'automation.label.coupling_radius' : 'automation.label.search_radius'}
          value={action.radius}
          maximum={action.kind === 'couple' ? couplingMaximum : sensorMaximum}
          onChange={(radius) => onChange({ ...action, radius })}
        />
      ) : null}
      {action.kind === 'change_depth' ? (
        <label className="automation-parameter">
          <span>{copy('automation.label.depth_direction')}</span>
          <select
            value={action.direction}
            onChange={(event) => onChange({
              ...action,
              direction: Number(event.target.value) as -1 | 0 | 1,
            })}
          >
            <option value={-1}>{copy('automation.depth.shallower')}</option>
            <option value={0}>{copy('automation.depth.hold')}</option>
            <option value={1}>{copy('automation.depth.deeper')}</option>
          </select>
        </label>
      ) : null}
      {action.kind === 'set_interface' ? (
        <label className="automation-toggle">
          <input
            type="checkbox"
            checked={action.open}
            onChange={(event) => onChange({ ...action, open: event.target.checked })}
          />
          <span>{copy(action.open ? 'automation.interface.open' : 'automation.interface.closed')}</span>
        </label>
      ) : null}
      {action.kind === 'set_route' ? (
        <>
          <RouteSelect
            value={action.route}
            routes={routes}
            onChange={(route) => {
              const selected = routes.find((held) => held.route === route);
              onChange({
                ...action,
                route,
                enabled: selected?.automationEnabled ?? action.enabled,
                capacity_limit: selected?.capacityLimit ?? selected?.capacity ?? action.capacity_limit,
                allocation_weight: selected?.allocationWeight ?? action.allocation_weight,
              });
            }}
          />
          <label className="automation-toggle">
            <input
              type="checkbox"
              checked={action.enabled}
              onChange={(event) => onChange({ ...action, enabled: event.target.checked })}
            />
            <span>{copy(action.enabled ? 'automation.route.enabled' : 'automation.route.disabled')}</span>
          </label>
          <UnitInput
            label="automation.label.route_limit"
            value={action.capacity_limit}
            maximum={routes.find((route) => route.route === action.route)?.capacity ?? 0}
            onChange={(capacity_limit) => onChange({ ...action, capacity_limit })}
          />
          <label className="automation-parameter">
            <span>{copy('automation.label.allocation_weight')}</span>
            <input
              type="number"
              min="1"
              max="65535"
              step="1"
              value={action.allocation_weight}
              onChange={(event) => onChange({
                ...action,
                allocation_weight: integer(event.target.value, 1),
              })}
            />
          </label>
        </>
      ) : null}
      {action.kind === 'emit_signal' ? (
        <UnitInput
          label="automation.label.signal_strength"
          value={action.strength}
          minimum={1}
          maximum={signalMaximum}
          onChange={(strength) => onChange({ ...action, strength })}
        />
      ) : null}
    </div>
  );
}

function UnitInput({
  label,
  value,
  minimum = 0,
  maximum = Number.MAX_SAFE_INTEGER,
  onChange,
}: {
  label: string;
  value: number;
  minimum?: number;
  maximum?: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="automation-parameter">
      <span>{copy(label)}</span>
      <input
        type="number"
        min={minimum / WHOLE}
        max={maximum / WHOLE}
        step="0.25"
        value={fixed(value)}
        onChange={(event) => onChange(rawUnits(event.target.value, minimum, maximum))}
      />
    </label>
  );
}

function RouteSelect({
  value,
  routes,
  onChange,
}: {
  value: number;
  routes: readonly FrameRoute[];
  onChange: (route: number) => void;
}) {
  return (
    <label className="automation-parameter">
      <span>{copy('automation.label.route')}</span>
      <select value={value} onChange={(event) => onChange(Number(event.target.value))}>
        {routes.map((route) => (
          <option key={route.route} value={route.route}>
            {copy('automation.label.route')} {route.route}: {route.tail} {copy('automation.label.to')} {route.head}
          </option>
        ))}
      </select>
    </label>
  );
}

interface AutomationWorkbenchProps {
  contract: ContractCatalogEntry | null;
  identity: RunIdentity | null;
  mode: FrameMode | null;
  rate: 1 | 4 | 16;
  step: number;
  queue: QueueState;
  tool: StillTool;
  policy: FrozenLocalPolicy;
  routeDefaults: readonly RouteControlDefault[];
  selection: FieldInspection | null;
  routes: readonly FrameRoute[];
  criterion: CriterionReading | null;
  pressures: readonly PressureState[];
  mechanismEvents: readonly MechanismTimelineEntry[];
  commissionHistory: readonly CommissionAttemptRecord[];
  commissionArchiveError: boolean;
  qualificationRequestArchiveError: boolean;
  qualificationExecutionArchiveError: boolean;
  qualificationJob: QualificationJob | null;
  qualificationTrialArtifacts: readonly QualificationTrialArtifact[];
  qualificationCriterionDecisions: readonly QualificationCriterionDecision[];
  qualificationFunctionDecision: QualificationFunctionDecision | null;
  qualificationGrades: readonly QualificationGrade[];
  qualificationFailureTrace: QualificationFailureTrace | null;
  qualificationResult: QualificationResultGroup | null;
  qualificationReceipt: QualificationUnlockReceipt | null;
  blueprints: readonly EngineeringBlueprintEntry[];
  generatorSources: readonly EngineeringGeneratorSourceEntry[];
  generatorSourceReadError: boolean;
  engineeringMigration: EngineeringMigrationJournal | null;
  engineeringRecoveries: readonly EngineeringOperationRecovery[];
  commissionBreakpoint: CommissionBreakpoint | null;
  commissionBreakpointHit: MechanismTimelineEntry | null;
  inspectionStep: number | null;
  selectedEventOrdinal: number | null;
  onDesign: () => void;
  onCommission: () => void;
  onPreviewRestart: () => Promise<ResponseEnvelope>;
  onPreviewQualification: () => Promise<ResponseEnvelope>;
  onFreezeQualification: (preview: QualificationInputPreview) => Promise<ResponseEnvelope>;
  onRetryQualificationPersistence: () => Promise<void>;
  onRetryQualificationExecutionPersistence: () => Promise<void>;
  onStartQualification: () => Promise<ResponseEnvelope>;
  onCancelQualification: () => Promise<ResponseEnvelope>;
  onResolveQualification: () => Promise<ResponseEnvelope>;
  onGradeQualification: () => Promise<ResponseEnvelope>;
  onTraceQualificationFailure: () => Promise<ResponseEnvelope>;
  onAssembleQualificationResult: () => Promise<ResponseEnvelope>;
  onProjectQualificationProgress: () => Promise<ResponseEnvelope>;
  onCaptureBlueprint: (name: string) => Promise<ResponseEnvelope>;
  onReadEngineeringAssembly: () => Promise<ResponseEnvelope>;
  onPreviewEngineeringAssembly: (
    draft: EngineeringAssemblyDraft,
  ) => Promise<ResponseEnvelope>;
  onAssemblyPreviewChange: (preview: EngineeringAssemblyPreview | null) => void;
  onCommitEngineeringAssembly: (
    draft: EngineeringAssemblyDraft,
    preview: EngineeringAssemblyPreview,
  ) => Promise<ResponseEnvelope>;
  onPreviewEngineeringTransition: (
    operation: EngineeringTransitionKind,
    entry: EngineeringGeneratorSourceEntry | null,
  ) => Promise<ResponseEnvelope>;
  onTransitionPreviewChange: (preview: EngineeringRunTransitionPreview | null) => void;
  onCommitEngineeringTransition: (
    preview: EngineeringRunTransitionPreview,
  ) => Promise<ResponseEnvelope>;
  onRestart: (preview: CommissionRestartPreview) => Promise<ResponseEnvelope>;
  onRate: (rate: 1 | 4 | 16) => void;
  onTool: (tool: StillTool) => void;
  onUndo: () => void;
  onCommit: () => Promise<void>;
  onDeployJunction: () => void;
  onOpenContracts: () => Promise<void>;
  onOpenLab: () => void;
  onSetBreakpoint: (breakpoint: CommissionBreakpoint | null) => void;
  onPreviewPolicy: (
    address: number,
    policy: FrozenLocalPolicy,
    routeDefaults: RouteControlDefault[],
  ) => Promise<ResponseEnvelope>;
  onPreviewChange: (preview: PolicyPreview | null) => void;
  onSelectEvent: (entry: MechanismTimelineEntry) => void;
  onApplyPolicy: (
    policy: FrozenLocalPolicy,
    routeDefaults: RouteControlDefault[],
  ) => Promise<ResponseEnvelope>;
}

function criterionThreshold(contractCriterion: ContractCatalogEntry['criteria'][number]): string {
  if (contractCriterion.metric === 'stored_charge') {
    return `${fixed(contractCriterion.threshold)} ${copy('unit.cu')}`;
  }
  if (contractCriterion.metric === 'accepted_flow') {
    return `${fixed(contractCriterion.threshold)} ${copy('unit.cu_per_step')}`;
  }
  if (contractCriterion.metric === 'leakage_ratio') {
    return `${(contractCriterion.threshold * 100 / WHOLE).toFixed(1)}${copy('unit.percent')}`;
  }
  return `${contractCriterion.threshold.toLocaleString('en-US')} ${copy('unit.steps')}`;
}

function criterionValue(metric: ContractCatalogEntry['criteria'][number]['metric'], value: number): string {
  if (metric === 'stored_charge') return `${fixed(value)} ${copy('unit.cu')}`;
  if (metric === 'accepted_flow') return `${fixed(value)} ${copy('unit.cu_per_step')}`;
  if (metric === 'leakage_ratio') return `${(value * 100 / WHOLE).toFixed(1)}${copy('unit.percent')}`;
  return `${value.toLocaleString('en-US')} ${copy('unit.steps')}`;
}

function criterionSource(contractCriterion: ContractCatalogEntry['criteria'][number]): string {
  const source = copy(`contract.source.${contractCriterion.source.kind}`);
  return contractCriterion.source.id === null
    ? source
    : `${source} ${contractCriterion.source.id}`;
}

function currentConstraint(
  contract: ContractCatalogEntry | null,
  reading: CriterionReading | null,
): string {
  if (!contract || !reading) return copy('automation.criterion.awaiting');
  for (const authored of contract.criteria) {
    if (authored.metric === 'stored_charge' && authored.source.id !== null) {
      const component = reading.components.find((held) => held.node === authored.source.id);
      if (component && !component.met) {
        return `${copy(`contract.metric.${authored.metric}`)} · ${criterionSource(authored)} · ${fixed(component.minimum_q)} / ${criterionThreshold(authored)}`;
      }
    } else if (authored.metric === 'accepted_flow' && authored.source.id !== null) {
      const route = reading.routes.find((held) => held.route === authored.source.id);
      if (route && !route.met) {
        return `${copy(`contract.metric.${authored.metric}`)} · ${criterionSource(authored)} · ${fixed(route.minimum)} / ${criterionThreshold(authored)}`;
      }
    } else if (authored.metric === 'leakage_ratio' && !reading.leakage.met) {
      const ratio = reading.leakage.ratio === null
        ? copy('criterion.unbounded')
        : `${(reading.leakage.ratio * 100 / WHOLE).toFixed(1)}${copy('unit.percent')}`;
      return `${copy(`contract.metric.${authored.metric}`)} · ${ratio} / ${criterionThreshold(authored)}`;
    } else if (authored.metric === 'hands_off_steps' && !reading.hands_off) {
      const retained = Math.max(0, authored.threshold - reading.hands_off_remaining);
      return `${copy(`contract.metric.${authored.metric}`)} · ${retained.toLocaleString('en-US')} / ${criterionThreshold(authored)}`;
    }
  }
  return copy(reading.all_metrics_met
    ? 'automation.criterion.current_met'
    : 'automation.criterion.observing');
}

interface CriterionMarginRow {
  id: string;
  label: string;
  measured: string;
  required: string;
  met: boolean;
  normalized: number;
  trend: 'awaiting' | 'improving' | 'steady' | 'weakening';
}

function marginTrend(current: number, prior: number | null, higherIsBetter: boolean): CriterionMarginRow['trend'] {
  if (prior === null) return 'awaiting';
  if (current === prior) return 'steady';
  const improving = higherIsBetter ? current > prior : current < prior;
  return improving ? 'improving' : 'weakening';
}

function criterionMarginRows(
  reading: CriterionReading,
  prior: CriterionReading | null,
): CriterionMarginRow[] {
  const rows: CriterionMarginRow[] = reading.components.map((component) => {
    const previous = prior?.components.find((held) => held.node === component.node) ?? null;
    return {
      id: `component:${component.node}`,
      label: `${copy('contract.source.component')} ${component.node}`,
      measured: `${fixed(component.charge)} ${copy('unit.cu')}`,
      required: `${copy('contract.comparison.at_least')} ${fixed(component.minimum_q)} ${copy('unit.cu')}`,
      met: component.met,
      normalized: component.margin / Math.max(1, Math.abs(component.minimum_q)),
      trend: marginTrend(component.charge, previous?.charge ?? null, true),
    };
  });
  rows.push(...reading.routes.map((route) => {
    const previous = prior?.routes.find((held) => held.route === route.route) ?? null;
    return {
      id: `route:${route.route}`,
      label: `${copy('contract.source.route')} ${route.route}`,
      measured: `${fixed(route.minimum)} ${copy('unit.cu_per_step')}`,
      required: `${copy('contract.comparison.at_least')} ${fixed(route.floor)} ${copy('unit.cu_per_step')}`,
      met: route.met,
      normalized: (route.minimum - route.floor) / Math.max(1, Math.abs(route.floor)),
      trend: marginTrend(route.minimum, previous?.minimum ?? null, true),
    };
  }));
  const leakageRatio = reading.leakage.ratio;
  const priorLeakage = prior?.leakage.ratio ?? null;
  rows.push({
    id: 'leakage',
    label: copy('automation.criterion.leakage'),
    measured: leakageRatio === null ? copy('automation.criterion.awaiting') : `${(leakageRatio * 100 / WHOLE).toFixed(1)}${copy('unit.percent')}`,
    required: `${copy('contract.comparison.at_most')} ${(reading.leakage.ceiling * 100 / WHOLE).toFixed(1)}${copy('unit.percent')}`,
    met: reading.leakage.met,
    normalized: leakageRatio === null
      ? Number.POSITIVE_INFINITY
      : (reading.leakage.ceiling - leakageRatio) / Math.max(1, reading.leakage.ceiling),
    trend: leakageRatio === null ? 'awaiting' : marginTrend(leakageRatio, priorLeakage, false),
  });
  const handsOffRequired = reading.hands_off_streak + reading.hands_off_remaining;
  rows.push({
    id: 'hands_off',
    label: copy('contract.metric.hands_off_steps'),
    measured: `${reading.hands_off_streak} ${copy('unit.steps')}`,
    required: `${copy('contract.comparison.at_least')} ${handsOffRequired} ${copy('unit.steps')}`,
    met: reading.hands_off,
    normalized: handsOffRequired === 0 ? 0 : -reading.hands_off_remaining / handsOffRequired,
    trend: marginTrend(reading.hands_off_streak, prior?.hands_off_streak ?? null, true),
  });
  return rows;
}

function CriterionMarginRail({
  reading,
  prior,
}: {
  reading: CriterionReading | null;
  prior: CriterionReading | null;
}) {
  if (!reading) return null;
  const rows = criterionMarginRows(reading, prior);
  const weakest = [...rows].sort((left, right) => left.normalized - right.normalized)[0]?.id;
  return (
    <details className="automation-margin-rail" open>
      <summary>
        <span>{copy('automation.margin.title')}</span>
        <strong>{copy(`automation.criterion.${reading.status}`)}</strong>
      </summary>
      <ol>
        {rows.map((row) => (
          <li key={row.id} data-met={row.met} data-weakest={row.id === weakest}>
            <span>{row.label}</span>
            <strong>{row.measured}</strong>
            <small>{row.required}</small>
            <b>{copy(`automation.margin.trend.${row.trend}`)}</b>
          </li>
        ))}
      </ol>
      <p>{copy('automation.margin.provisional')}</p>
    </details>
  );
}

interface GuidanceCue {
  id: string;
  key: string;
  address: number | null;
}

function contextualGuidance(
  contract: ContractCatalogEntry | null,
  policy: FrozenLocalPolicy,
  criterion: CriterionReading | null,
  events: readonly MechanismTimelineEntry[],
  selection: FieldInspection | null,
): GuidanceCue | null {
  if (!contract) return null;
  if (policy.components.length === 0) {
    return {
      id: `${contract.id}:policy`,
      key: contract.id === 'transfer'
        ? 'automation.guidance.transfer_no_policy'
        : contract.id === 'buffer'
          ? 'automation.guidance.buffer_no_policy'
          : 'automation.guidance.no_policy',
      address: null,
    };
  }
  if (contract.id === 'buffer') {
    const selectedClosed = selection?.target === 'node' && !selection.open
      ? selection.id
      : null;
    const latestInterface = [...events].reverse().find((entry) => (
      entry.event.kind === 'interface'
    ));
    const closedAddress = selectedClosed
      ?? (latestInterface?.event.kind === 'interface' && !latestInterface.event.open
        ? latestInterface.event.node
        : null);
    if (closedAddress !== null) {
      return {
        id: `${contract.id}:interface:${closedAddress}`,
        key: 'automation.guidance.buffer_open',
        address: closedAddress,
      };
    }
    const latestSupply = [...events].reverse().find((entry) => entry.event.kind === 'supply');
    const emitting = latestSupply?.event.kind === 'supply'
      ? latestSupply.event.emitting
      : null;
    if (emitting && selection?.target === 'form'
        && selection.ability === 'reserve_discharge'
        && selection.ability_value < selection.ability_limit) {
      return {
        id: `${contract.id}:fill:${selection.id}`,
        key: contract.guidance_keys[0] ?? 'automation.guidance.buffer_fill',
        address: selection.node,
      };
    }
    const recentRoute = [...events].reverse().find((entry) => (
      entry.event.kind === 'route' && entry.event.state !== 'flowing'
    ));
    if (emitting === false && recentRoute?.event.kind === 'route') {
      return {
        id: `${contract.id}:bridge:${recentRoute.event.route}:${recentRoute.event.state}`,
        key: contract.guidance_keys[1] ?? 'automation.guidance.buffer_bridge',
        address: null,
      };
    }
    const bufferReceiver = criterion?.components.find((component) => !component.met);
    if (bufferReceiver && criterion && criterion.observed_steps > 0) {
      return {
        id: `${contract.id}:recover:${bufferReceiver.node}`,
        key: contract.guidance_keys[2] ?? 'automation.guidance.buffer_recover',
        address: bufferReceiver.node,
      };
    }
    return null;
  }
  if (contract.id === 'transfer') {
    const selectedClosed = selection?.target === 'node' && !selection.open
      ? selection.id
      : null;
    const latestInterface = [...events].reverse().find((entry) => (
      entry.event.kind === 'interface'
    ));
    const closedAddress = selectedClosed
      ?? (latestInterface?.event.kind === 'interface' && !latestInterface.event.open
        ? latestInterface.event.node
        : null);
    if (closedAddress !== null) {
      return {
        id: `${contract.id}:interface:${closedAddress}`,
        key: contract.guidance_keys[0] ?? 'automation.guidance.interface_closed',
        address: closedAddress,
      };
    }
    const requiredRoute = contract.criteria.find((authored) => (
      authored.metric === 'accepted_flow' && authored.source.id !== null
    ))?.source.id ?? null;
    const recentRoute = [...events].reverse().find((entry) => (
      entry.event.kind === 'route'
      && (requiredRoute === null || entry.event.route === requiredRoute)
    ));
    const routeReading = selection?.target === 'route'
      ? { route: selection.id, state: selection.outcome }
      : recentRoute?.event.kind === 'route'
        ? { route: recentRoute.event.route, state: recentRoute.event.state }
        : null;
    if (routeReading && routeReading.state !== 'flowing') {
      return {
        id: `${contract.id}:route:${routeReading.route}:${routeReading.state}`,
        key: contract.guidance_keys[1] ?? 'automation.guidance.route_limited',
        address: null,
      };
    }
    const transferReceiver = criterion?.components.find((component) => !component.met);
    if (transferReceiver && criterion && criterion.observed_steps > 0) {
      return {
        id: `${contract.id}:receiver:${transferReceiver.node}`,
        key: contract.guidance_keys[2] ?? 'automation.guidance.receiver_floor',
        address: transferReceiver.node,
      };
    }
    return null;
  }
  const recentPolicy = events
    .filter((entry) => entry.event.kind === 'policy')
    .slice(-8);
  const latest = recentPolicy.at(-1)?.event;
  const selectedRuntime = selection?.target === 'form' || selection?.target === 'node'
    ? selection
    : null;
  const selectedAddress = selectedRuntime?.target === 'form'
    ? selectedRuntime.node
    : selectedRuntime?.target === 'node'
      ? selectedRuntime.id
      : null;
  if (selectedRuntime?.policy_outcome === 'out_of_range'
      || latest?.kind === 'policy' && latest.outcome === 'out_of_range') {
    const address = selectedRuntime?.policy_outcome === 'out_of_range'
      ? selectedAddress
      : latest?.kind === 'policy'
        ? latest.address
        : null;
    return {
      id: `${contract.id}:range:${address ?? 'unknown'}`,
      key: contract.guidance_keys[1] ?? 'automation.guidance.out_of_range',
      address,
    };
  }
  if (selectedRuntime?.policy_outcome === 'no_target' && selectedRuntime.policy_timer >= 3) {
    return {
      id: `${contract.id}:target:${selectedAddress ?? 'unknown'}`,
      key: contract.guidance_keys[0] ?? 'automation.guidance.no_target',
      address: selectedAddress,
    };
  }
  if (latest?.kind === 'policy' && latest.outcome === 'no_target') {
    const repeated = recentPolicy.filter((entry) => (
      entry.event.kind === 'policy'
      && entry.event.address === latest.address
      && entry.event.outcome === 'no_target'
    )).length;
    if (repeated >= 3) {
      return {
        id: `${contract.id}:target:${latest.address}`,
        key: contract.guidance_keys[0] ?? 'automation.guidance.no_target',
        address: latest.address,
      };
    }
  }
  const receiver = criterion?.components.find((component) => !component.met);
  if (receiver && criterion && criterion.observed_steps > 0) {
    return {
      id: `${contract.id}:receiver:${receiver.node}`,
      key: contract.guidance_keys[2] ?? 'automation.guidance.receiver_floor',
      address: receiver.node,
    };
  }
  return null;
}

function QualificationPreview({
  preview,
  frozen,
  loading,
  error,
  stale,
  persistenceError,
  executionPersistenceError,
  job,
  trialArtifacts,
  criterionDecisions,
  functionDecision,
  grades,
  failureTrace,
  result,
  receipt,
  onRefresh,
  onFreeze,
  onRetryPersistence,
  onRetryExecutionPersistence,
  onStartQualification,
  onCancelQualification,
  onResolveQualification,
  onGradeQualification,
  onTraceQualificationFailure,
  onAssembleQualificationResult,
  onProjectQualificationProgress,
}: {
  preview: QualificationInputPreview | null;
  frozen: RunIdentity['qualificationRequest'];
  loading: boolean;
  error: boolean;
  stale: boolean;
  persistenceError: boolean;
  executionPersistenceError: boolean;
  job: QualificationJob | null;
  trialArtifacts: readonly QualificationTrialArtifact[];
  criterionDecisions: readonly QualificationCriterionDecision[];
  functionDecision: QualificationFunctionDecision | null;
  grades: readonly QualificationGrade[];
  failureTrace: QualificationFailureTrace | null;
  result: QualificationResultGroup | null;
  receipt: QualificationUnlockReceipt | null;
  onRefresh: () => void;
  onFreeze: (preview: QualificationInputPreview) => Promise<ResponseEnvelope>;
  onRetryPersistence: () => Promise<void>;
  onRetryExecutionPersistence: () => Promise<void>;
  onStartQualification: () => Promise<ResponseEnvelope>;
  onCancelQualification: () => Promise<ResponseEnvelope>;
  onResolveQualification: () => Promise<ResponseEnvelope>;
  onGradeQualification: () => Promise<ResponseEnvelope>;
  onTraceQualificationFailure: () => Promise<ResponseEnvelope>;
  onAssembleQualificationResult: () => Promise<ResponseEnvelope>;
  onProjectQualificationProgress: () => Promise<ResponseEnvelope>;
}) {
  const [freezeState, setFreezeState] = useState<'idle' | 'confirming' | 'submitting'>('idle');
  const [freezeStatus, setFreezeStatus] = useState<string | null>(null);
  const [jobAction, setJobAction] = useState<'idle' | 'starting' | 'canceling' | 'resolving' | 'grading' | 'tracing' | 'assembling' | 'projecting'>('idle');
  const [jobStatus, setJobStatus] = useState<string | null>(null);
  const input = frozen?.input ?? preview?.input ?? null;
  const prospectiveReceiptLabels = input ? [
    ...input.prospective_receipt.actions.map((value) => copy(`automation.action.${value}`)),
    ...input.prospective_receipt.conditions.map((value) => copy(`automation.condition.${value}`)),
    ...input.prospective_receipt.hardware.map((value) => copy(`contract.hardware.${value}`)),
    ...(input.prospective_receipt.next_contract
      ? [copy('automation.qualification.next_contract')]
      : []),
  ] : [];
  const state = frozen
    ? persistenceError || executionPersistenceError ? 'frozen_degraded' : 'frozen'
    : stale
    ? 'stale'
    : loading
      ? 'loading'
      : error
        ? 'error'
        : preview?.status ?? 'idle';
  const freeze = async (): Promise<void> => {
    if (!preview || preview.status !== 'complete' || stale || freezeState !== 'confirming') return;
    setFreezeState('submitting');
    setFreezeStatus(copy('automation.qualification.freezing'));
    const answer = await onFreeze(preview);
    if (!answer.ok) {
      setFreezeState('confirming');
      setFreezeStatus(copy('automation.qualification.freeze_refused'));
      return;
    }
    setFreezeState('idle');
    setFreezeStatus(null);
  };
  const startQualification = async (): Promise<void> => {
    if (!frozen || persistenceError || executionPersistenceError || jobAction !== 'idle') return;
    setJobAction('starting');
    setJobStatus(copy('automation.qualification.job_preparing'));
    const answer = await onStartQualification();
    setJobAction('idle');
    setJobStatus(answer.ok
      ? copy('automation.qualification.job_dispatched')
      : copy('automation.qualification.job_refused'));
  };
  const cancelQualification = async (): Promise<void> => {
    if (!job || jobAction !== 'idle') return;
    setJobAction('canceling');
    setJobStatus(copy('automation.qualification.job_canceling'));
    const answer = await onCancelQualification();
    setJobAction('idle');
    setJobStatus(answer.ok
      ? copy('automation.qualification.job_cancel_requested')
      : copy('automation.qualification.job_cancel_refused'));
  };
  const resolveQualification = async (): Promise<void> => {
    if (!job || job.status !== 'completed' || functionDecision || jobAction !== 'idle') return;
    setJobAction('resolving');
    setJobStatus(copy('automation.qualification.resolution_running'));
    const answer = await onResolveQualification();
    setJobAction('idle');
    setJobStatus(answer.ok
      ? copy('automation.qualification.resolution_retained')
      : copy('automation.qualification.resolution_refused'));
  };
  const gradeQualification = async (): Promise<void> => {
    if (!functionDecision || grades.length > 0 || jobAction !== 'idle') return;
    setJobAction('grading');
    setJobStatus(copy('automation.qualification.grading_running'));
    const answer = await onGradeQualification();
    setJobAction('idle');
    setJobStatus(answer.ok
      ? copy('automation.qualification.grading_retained')
      : copy('automation.qualification.grading_refused'));
  };
  const traceQualificationFailure = async (): Promise<void> => {
    if (!functionDecision || functionDecision.definition.passed || failureTrace || jobAction !== 'idle') return;
    setJobAction('tracing');
    setJobStatus(copy('automation.qualification.trace_running'));
    const answer = await onTraceQualificationFailure();
    setJobAction('idle');
    setJobStatus(answer.ok
      ? copy('automation.qualification.trace_retained')
      : copy('automation.qualification.trace_refused'));
  };
  const assembleQualificationResult = async (): Promise<void> => {
    if (!functionDecision || grades.length !== 4 || result || jobAction !== 'idle') return;
    if (!functionDecision.definition.passed && !failureTrace) return;
    setJobAction('assembling');
    setJobStatus(copy('automation.qualification.result_assembling'));
    const answer = await onAssembleQualificationResult();
    setJobAction('idle');
    setJobStatus(answer.ok
      ? copy('automation.qualification.result_retained')
      : copy('automation.qualification.result_refused'));
  };
  const projectQualificationProgress = async (): Promise<void> => {
    if (!result || result.result.definition.outcome !== 'passed' || receipt || jobAction !== 'idle') return;
    setJobAction('projecting');
    setJobStatus(copy('automation.qualification.projection_running'));
    const answer = await onProjectQualificationProgress();
    setJobAction('idle');
    setJobStatus(answer.ok
      ? copy('automation.qualification.projection_retained')
      : copy('automation.qualification.projection_refused'));
  };
  const violatedDecision = failureTrace
    ? criterionDecisions.find((decision) => (
      decision.decision_id === failureTrace.definition.criterion_decision_id
    )) ?? null
    : null;
  const completedTrials = new Set(trialArtifacts
    .filter((artifact) => !job || artifact.job_id === job.job_id)
    .map((artifact) => artifact.trial));
  const currentTrial = job && job.status === 'running'
    ? Array.from({ length: job.trial_count }, (_, trial) => trial)
      .find((trial) => !completedTrials.has(trial)) ?? null
    : null;
  return (
    <details className="automation-qualification" data-state={state}>
      <summary>
        <span>{copy(frozen ? 'automation.qualification.frozen_title' : 'automation.qualification.title')}</span>
        <strong>{input
          ? `${input.procedure.trial_count} × ${Number(input.procedure.schedule.duration_steps ?? 0).toLocaleString('en-US')} ${copy('unit.steps')}`
          : copy(`automation.qualification.state.${state}`)}</strong>
      </summary>
      <p>{copy(frozen ? 'automation.qualification.frozen_boundary' : 'automation.qualification.preview_only')}</p>
      {!frozen && loading ? <p role="status">{copy('automation.qualification.loading')}</p> : null}
      {!frozen && error ? <p role="alert">{copy('automation.qualification.error')}</p> : null}
      {!frozen && stale ? <p role="alert">{copy('automation.qualification.stale')}</p> : null}
      {frozen ? (
        <section className="automation-qualification-seal" data-persistence={persistenceError ? 'degraded' : 'durable'}>
          <h3>{copy('automation.qualification.seal')}</h3>
          <dl>
            <div><dt>{copy('automation.qualification.request_id')}</dt><dd><code>{frozen.request_id}</code></dd></div>
            <div><dt>{copy('automation.qualification.source_branch')}</dt><dd><code>{frozen.input.branch_id ?? copy('automation.identity.pending')}</code></dd></div>
            <div><dt>{copy('automation.qualification.persistence')}</dt><dd>{copy(persistenceError ? 'automation.qualification.persistence_degraded' : 'automation.qualification.persistence_durable')}</dd></div>
            <div><dt>{copy('automation.qualification.execution')}</dt><dd>{copy(job ? `automation.qualification.job_status.${job.status}` : 'automation.qualification.execution_not_started')}</dd></div>
          </dl>
        </section>
      ) : null}
      {input ? (
        <>
          <section className="automation-identity">
            <h3>{copy('automation.identity.title')}</h3>
            <dl>
              <div><dt>{copy('automation.identity.attempt')}</dt><dd><code>{input.attempt_id ?? copy('automation.identity.pending')}</code></dd></div>
              <div><dt>{copy('automation.identity.branch_id')}</dt><dd><code>{input.branch_id ?? copy('automation.identity.pending')}</code></dd></div>
              <div><dt>{copy('automation.identity.parent_branch')}</dt><dd><code>{input.parent_branch_id ?? copy('automation.identity.pending')}</code></dd></div>
              <div><dt>{copy('automation.identity.branch_nonce')}</dt><dd>{input.branch_nonce}</dd></div>
              <div><dt>{copy('contract.ladder.generator')}</dt><dd><code>{input.generator_spec_hash}</code></dd></div>
              <div><dt>{copy('contract.ladder.assembly')}</dt><dd><code>{input.assembly_template_hash}</code></dd></div>
              <div><dt>{copy('contract.ladder.regime')}</dt><dd>{input.regime}</dd></div>
              <div><dt>{copy(frozen ? 'automation.qualification.request_id' : 'automation.qualification.preview_hash')}</dt><dd><code>{frozen?.request_id ?? preview?.preview_hash}</code></dd></div>
            </dl>
          </section>
          <section>
            <h3>{copy('automation.qualification.procedure')}</h3>
            <dl>
              <div><dt>{copy('contract.ladder.trials')}</dt><dd>{input.procedure.trial_count}</dd></div>
              <div><dt>{copy('contract.ladder.duration')}</dt><dd>{Number(input.procedure.schedule.duration_steps ?? 0).toLocaleString('en-US')} {copy('unit.steps')}</dd></div>
              <div><dt>{copy('automation.qualification.grace')}</dt><dd>{input.criterion_vector.failure_grace_steps} {copy('unit.steps')}</dd></div>
              <div><dt>{copy('automation.qualification.control')}</dt><dd>{copy('automation.qualification.control_hands_off')}</dd></div>
              <div><dt>{copy('automation.qualification.schedule')}</dt><dd><code>{input.procedure.schedule_hash}</code></dd></div>
              <div><dt>{copy('automation.qualification.rng')}</dt><dd>{input.procedure.rng_algorithm}</dd></div>
              <div><dt>{copy('automation.qualification.seed_custody')}</dt><dd>{copy('automation.qualification.seed_request_address')}</dd></div>
              <div><dt>{copy('automation.qualification.retention')}</dt><dd>{copy('automation.qualification.retention_exact')}</dd></div>
            </dl>
          </section>
          <ol>
            {input.criterion_vector.criteria.map((authored) => (
              <li key={authored.id}>
                <span>{criterionSource(authored)}</span>
                <strong>{copy(`contract.metric.${authored.metric}`)}</strong>
                <b>{copy(`contract.comparison.${authored.comparison}`)} {criterionThreshold(authored)}</b>
                <small>{copy(`contract.aggregation.${authored.aggregation}`)} · {authored.window_steps} {copy('unit.steps')}</small>
              </li>
            ))}
          </ol>
          <section>
            <h3>{copy('contract.ladder.grade_bands')}</h3>
            <dl>
              {(['throughput', 'resilience', 'economy', 'complexity'] as const).map((axis) => (
                <div key={axis}>
                  <dt>{copy(`contract.grade.${axis}`)}</dt>
                  <dd>{copy(`automation.qualification.evidence.${axis}`)} · {input.grade_axes[axis].bands.map((value) => Math.round(value * 100 / WHOLE)).join(' / ')}</dd>
                </div>
              ))}
            </dl>
          </section>
          <section>
            <h3>{copy('contract.ladder.unlocks')}</h3>
            <p>{prospectiveReceiptLabels.join(' · ') || copy('automation.qualification.no_receipt')}</p>
          </section>
          {!frozen && preview?.missing_inputs.length ? (
            <section>
              <h3>{copy('automation.qualification.missing')}</h3>
              <ul>
                {preview.missing_inputs.map((missing) => (
                  <li key={missing}>{copy(`automation.qualification.missing.${missing}`)}</li>
                ))}
              </ul>
            </section>
          ) : null}
        </>
      ) : null}
      {frozen ? (
        <>
          <section className="automation-qualification-job" data-state={job?.status ?? 'not_started'}>
            <h3>{copy('automation.qualification.job_title')}</h3>
            {job ? (
              <>
                <dl>
                  <div><dt>{copy('automation.qualification.job_id')}</dt><dd><code>{job.job_id}</code></dd></div>
                  <div><dt>{copy('automation.qualification.job_progress')}</dt><dd>{completedTrials.size} / {job.trial_count}</dd></div>
                </dl>
                <div className="automation-trial-matrix" aria-label={copy('automation.qualification.job_trials')}>
                  {Array.from({ length: job.trial_count }, (_, trial) => (
                    <span
                      key={trial}
                      data-state={completedTrials.has(trial)
                        ? 'completed'
                        : currentTrial === trial ? 'running' : 'queued'}
                    >
                      <b>{trial + 1}</b>
                      <small>{copy(completedTrials.has(trial)
                        ? 'automation.qualification.trial.completed'
                        : currentTrial === trial
                          ? 'automation.qualification.trial.running'
                          : 'automation.qualification.trial.queued')}</small>
                    </span>
                  ))}
                </div>
              </>
            ) : (
              <p>{copy('automation.qualification.job_not_started')}</p>
            )}
            {jobStatus ? <p role="status">{jobStatus}</p> : null}
          </section>
          {functionDecision ? (
            <section
              className="automation-qualification-resolution"
              data-result={functionDecision.definition.status}
            >
              <header>
                <span>{copy('automation.qualification.resolution_title')}</span>
                <strong>{copy(`automation.qualification.resolution.${functionDecision.definition.status}`)}</strong>
              </header>
              <p>{copy('automation.qualification.resolution_boundary')}</p>
              <dl>
                <div>
                  <dt>{copy('automation.qualification.function_decision_id')}</dt>
                  <dd><code>{functionDecision.function_decision_id}</code></dd>
                </div>
                <div>
                  <dt>{copy('automation.qualification.resolved_relations')}</dt>
                  <dd>{criterionDecisions.length}</dd>
                </div>
              </dl>
              <ol>
                {criterionDecisions.map((decision) => {
                  const held = decision.definition;
                  return (
                    <li key={decision.decision_id} data-result={held.status}>
                      <span>{copy('automation.qualification.trial')} {held.trial + 1}</span>
                      <strong>{held.criterion_id}</strong>
                      <b>
                        {copy(`contract.comparison.${held.comparison}`)} {criterionValue(held.metric, held.threshold)}
                        {' · '}{copy('automation.qualification.measured')} {criterionValue(held.metric, held.measured)}
                      </b>
                      <small>
                        {copy(`automation.qualification.resolution.${held.status}`)}
                        {' · '}{held.window_start_step}–{held.window_end_step} {copy('unit.steps')}
                      </small>
                    </li>
                  );
                })}
              </ol>
            </section>
          ) : null}
          {grades.length > 0 ? (
            <section className="automation-qualification-grades">
              <header>
                <span>{copy('automation.qualification.grades_title')}</span>
                <strong>{copy('automation.qualification.grades_independent')}</strong>
              </header>
              <p>{copy('automation.qualification.grades_boundary')}</p>
              <div>
                {grades.map((grade) => {
                  const held = grade.definition;
                  return (
                    <article key={grade.grade_id} data-axis={held.axis}>
                      <header>
                        <strong>{copy(`contract.grade.${held.axis}`)}</strong>
                        <b>{copy('automation.qualification.grade_band')} {held.band} / 4</b>
                      </header>
                      <meter min={0} max={WHOLE} value={held.score} />
                      <span>{Math.round(held.score * 100 / WHOLE)}{copy('unit.percent')}</span>
                      <p>{copy(`automation.qualification.evidence.${held.axis}`)}</p>
                      <code>{grade.grade_id}</code>
                    </article>
                  );
                })}
              </div>
            </section>
          ) : null}
          {failureTrace ? (
            <section className="automation-qualification-trace" data-state={failureTrace.definition.status}>
              <header>
                <span>{copy('automation.qualification.trace_title')}</span>
                <strong>{copy(`automation.qualification.trace.${failureTrace.definition.status}`)}</strong>
              </header>
              <p>{copy('automation.qualification.trace_boundary')}</p>
              <dl>
                <div><dt>{copy('automation.qualification.trace_id')}</dt><dd><code>{failureTrace.failure_trace_id}</code></dd></div>
                <div><dt>{copy('automation.qualification.first_violated')}</dt><dd>{violatedDecision?.definition.criterion_id ?? copy('automation.identity.pending')}</dd></div>
                <div><dt>{copy('automation.qualification.trial')}</dt><dd>{failureTrace.definition.trial + 1}</dd></div>
                <div><dt>{copy('automation.qualification.resolution_step')}</dt><dd>{failureTrace.definition.resolution_step.toLocaleString('en-US')}</dd></div>
                <div><dt>{copy('automation.qualification.retained_steps')}</dt><dd>{failureTrace.definition.trace_steps.length}</dd></div>
                <div><dt>{copy('automation.qualification.retained_events')}</dt><dd>{failureTrace.definition.mechanism_events.length}</dd></div>
                <div><dt>{copy('automation.qualification.inference')}</dt><dd>{copy('automation.qualification.inference_none')}</dd></div>
              </dl>
              {violatedDecision ? (
                <p className="automation-trace-relation">
                  {copy(`contract.metric.${violatedDecision.definition.metric}`)}
                  {' · '}{copy(`contract.comparison.${violatedDecision.definition.comparison}`)}
                  {' '}{criterionValue(violatedDecision.definition.metric, violatedDecision.definition.threshold)}
                  {' · '}{copy('automation.qualification.measured')}
                  {' '}{criterionValue(violatedDecision.definition.metric, violatedDecision.definition.measured)}
                </p>
              ) : null}
            </section>
          ) : null}
          {result ? (
            <section className="automation-qualification-result" data-outcome={result.result.definition.outcome}>
              <header>
                <span>{copy('automation.qualification.result_title')}</span>
                <strong>{copy(`automation.qualification.result.${result.result.definition.outcome}`)}</strong>
              </header>
              <p>{copy('automation.qualification.result_boundary')}</p>
              <dl>
                <div><dt>{copy('automation.qualification.result_id')}</dt><dd><code>{result.result.result_id}</code></dd></div>
                <div><dt>{copy('automation.qualification.result_marker_id')}</dt><dd><code>{result.complete_marker.marker_id}</code></dd></div>
                <div><dt>{copy('automation.qualification.result_children')}</dt><dd>{result.complete_marker.definition.child_count}</dd></div>
                <div><dt>{copy('automation.qualification.execution_status')}</dt><dd>{copy('automation.qualification.execution_completed')}</dd></div>
                <div><dt>{copy('automation.qualification.result_protocol')}</dt><dd>{result.result.definition.protocol_version}</dd></div>
                <div><dt>{copy('automation.qualification.result_build')}</dt><dd><code>{result.result.definition.build.package} {result.result.definition.build.version}</code></dd></div>
              </dl>
            </section>
          ) : null}
          {receipt ? (
            <section className="automation-qualification-receipt">
              <header>
                <span>{copy('automation.qualification.receipt_title')}</span>
                <strong>{copy('automation.qualification.receipt_projected')}</strong>
              </header>
              <p>{copy('automation.qualification.receipt_boundary')}</p>
              <dl>
                <div><dt>{copy('automation.qualification.receipt_id')}</dt><dd><code>{receipt.receipt_id}</code></dd></div>
                <div><dt>{copy('automation.qualification.receipt_contract')}</dt><dd>{receipt.definition.contract_id}</dd></div>
                <div><dt>{copy('automation.qualification.receipt_result')}</dt><dd><code>{receipt.definition.result_id}</code></dd></div>
                <div><dt>{copy('automation.qualification.receipt_next')}</dt><dd>{receipt.definition.next_contract ?? copy('automation.qualification.receipt_none')}</dd></div>
              </dl>
              <div className="automation-receipt-capabilities">
                {receipt.definition.hardware.map((value) => (
                  <span key={`hardware:${value}`}>{copy(`contract.hardware.${value}`)}</span>
                ))}
                {receipt.definition.conditions.map((value) => (
                  <span key={`condition:${value}`}>{copy(`automation.condition.${value}`)}</span>
                ))}
                {receipt.definition.actions.map((value) => (
                  <span key={`action:${value}`}>{copy(`automation.action.${value}`)}</span>
                ))}
              </div>
            </section>
          ) : null}
          <div className="automation-qualification-actions">
            {persistenceError ? (
              <button type="button" onClick={() => { void onRetryPersistence(); }}>
                {copy('automation.qualification.retry_persistence')}
              </button>
            ) : null}
            {executionPersistenceError ? (
              <button type="button" onClick={() => { void onRetryExecutionPersistence(); }}>
                {copy('automation.qualification.retry_execution_persistence')}
              </button>
            ) : null}
            {!job || ['canceled', 'interrupted'].includes(job.status) ? (
              <button type="button" onClick={() => { void startQualification(); }} disabled={persistenceError || executionPersistenceError || jobAction !== 'idle'}>
                {copy(job && ['canceled', 'interrupted'].includes(job.status)
                  ? 'automation.qualification.job_resume'
                  : 'automation.qualification.job_start')}
              </button>
            ) : job.status === 'queued' ? (
              <button type="button" onClick={() => { void startQualification(); }} disabled={persistenceError || executionPersistenceError || jobAction !== 'idle'}>
                {copy('automation.qualification.job_dispatch')}
              </button>
            ) : job.status === 'running' || job.status === 'cancel_requested' ? (
              <button type="button" onClick={() => { void cancelQualification(); }} disabled={job.status === 'cancel_requested' || jobAction !== 'idle'}>
                {copy(job.status === 'cancel_requested' ? 'automation.qualification.job_cancel_requested' : 'automation.qualification.job_cancel')}
              </button>
            ) : job.status === 'completed' && !functionDecision ? (
              <button type="button" onClick={() => { void resolveQualification(); }} disabled={executionPersistenceError || jobAction !== 'idle'}>
                {copy('automation.qualification.resolve_criteria')}
              </button>
            ) : functionDecision && grades.length === 0 ? (
              <button type="button" onClick={() => { void gradeQualification(); }} disabled={executionPersistenceError || jobAction !== 'idle'}>
                {copy('automation.qualification.calculate_grades')}
              </button>
            ) : functionDecision
                && !functionDecision.definition.passed
                && grades.length === 4
                && !failureTrace ? (
              <button type="button" onClick={() => { void traceQualificationFailure(); }} disabled={executionPersistenceError || jobAction !== 'idle'}>
                {copy('automation.qualification.build_failure_trace')}
              </button>
            ) : functionDecision
                && grades.length === 4
                && (functionDecision.definition.passed || failureTrace)
                && !result ? (
              <button type="button" onClick={() => { void assembleQualificationResult(); }} disabled={executionPersistenceError || jobAction !== 'idle'}>
                {copy('automation.qualification.assemble_result')}
              </button>
            ) : result?.result.definition.outcome === 'passed' && !receipt ? (
              <button type="button" onClick={() => { void projectQualificationProgress(); }} disabled={executionPersistenceError || jobAction !== 'idle'}>
                {copy('automation.qualification.project_availability')}
              </button>
            ) : receipt ? (
              <button type="button" disabled>{copy('automation.qualification.projection_complete')}</button>
            ) : result?.result.definition.outcome === 'failed' ? (
              <button type="button" disabled>{copy('automation.qualification.projection_failed')}</button>
            ) : (
              <button type="button" disabled>{copy(grades.length > 0
                ? functionDecision?.definition.passed
                  ? 'automation.qualification.passing_result_unavailable'
                  : 'automation.qualification.failed_result_unavailable'
                : 'automation.qualification.job_invalid')}</button>
            )}
          </div>
        </>
      ) : (
        <>
          <div className="automation-qualification-actions">
            <button type="button" onClick={onRefresh} disabled={loading || freezeState === 'submitting'}>{copy('automation.qualification.refresh')}</button>
            <button
              type="button"
              onClick={() => setFreezeState('confirming')}
              disabled={loading || stale || preview?.status !== 'complete' || freezeState !== 'idle'}
            >
              {copy(
                stale
                  ? 'automation.qualification.unavailable_stale'
                  : preview?.status === 'complete'
                    ? 'automation.qualification.review_freeze'
                    : 'automation.qualification.unavailable_incomplete',
              )}
            </button>
          </div>
          {freezeState !== 'idle' ? (
            <section className="automation-qualification-freeze-confirm" aria-label={copy('automation.qualification.freeze_title')}>
              <h3>{copy('automation.qualification.freeze_title')}</h3>
              <p>{copy('automation.qualification.freeze_consequence')}</p>
              <ul>
                <li>{copy('automation.qualification.freeze_candidate')}</li>
                <li>{copy('automation.qualification.freeze_procedure')}</li>
                <li>{copy('automation.qualification.freeze_controls')}</li>
                <li>{copy('automation.qualification.freeze_no_execution')}</li>
              </ul>
              {freezeStatus ? <p role="status">{freezeStatus}</p> : null}
              <div>
                <button type="button" onClick={() => { setFreezeState('idle'); setFreezeStatus(null); }} disabled={freezeState === 'submitting'}>
                  {copy('automation.qualification.freeze_cancel')}
                </button>
                <button type="button" onClick={() => { void freeze(); }} disabled={freezeState === 'submitting'}>
                  {copy('automation.qualification.freeze_confirm')}
                </button>
              </div>
            </section>
          ) : null}
        </>
      )}
    </details>
  );
}

function shortIdentity(value: string | null | undefined): string {
  if (!value) return copy('automation.identity.pending');
  return value.length > 16 ? `${value.slice(0, 12)}...` : value;
}

function marginReading(record: CommissionAttemptRecord): string {
  const margin = record.weakestMargin;
  if (!margin) return copy('automation.attempts.margin_unavailable');
  const object = margin.objectId === null ? '' : ` ${margin.objectId}`;
  const measured = margin.kind === 'leakage'
    ? `${(margin.measured * 100 / WHOLE).toFixed(1)}${copy('unit.percent')}`
    : margin.kind === 'hands_off'
      ? `${margin.measured} ${copy('unit.steps')}`
      : `${fixed(margin.measured)} ${copy(margin.kind === 'route' ? 'unit.cu_per_step' : 'unit.cu')}`;
  const required = margin.kind === 'leakage'
    ? `${(margin.required * 100 / WHOLE).toFixed(1)}${copy('unit.percent')}`
    : margin.kind === 'hands_off'
      ? `${margin.required} ${copy('unit.steps')}`
      : `${fixed(margin.required)} ${copy(margin.kind === 'route' ? 'unit.cu_per_step' : 'unit.cu')}`;
  return `${copy(`automation.attempts.margin.${margin.kind}`)}${object}: ${measured} / ${required}`;
}

function AttemptHistory({
  records,
  durabilityError,
  currentIdentity,
  currentCriterion,
  currentEvents,
}: {
  records: readonly CommissionAttemptRecord[];
  durabilityError: boolean;
  currentIdentity: RunIdentity | null;
  currentCriterion: CriterionReading | null;
  currentEvents: readonly MechanismTimelineEntry[];
}) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selected = records.find((record) => record.id === selectedId) ?? records[0] ?? null;
  const difference = selected ? firstObservedDifference(selected.events, currentEvents) : null;
  return (
    <details className="automation-attempts">
      <summary>
        <span>{copy('automation.attempts.title')}</span>
        <strong>{records.length}</strong>
      </summary>
      {durabilityError ? <p role="status">{copy('automation.attempts.persistence_error')}</p> : null}
      {records.length === 0 ? <p>{copy('automation.attempts.empty')}</p> : (
        <ol>
          {records.slice(0, 12).map((record) => (
            <li key={record.id} data-closure={record.closure} data-selected={record.id === selected?.id}>
              <button type="button" className="automation-attempt-select" onClick={() => setSelectedId(record.id)}>
                <strong>{copy(`automation.attempts.closure.${record.closure}`)}</strong>
                <span>{copy('automation.attempts.steps')} {record.openingStep}-{record.closingStep}</span>
              </button>
              <dl>
                <div><dt>{copy('automation.identity.attempt')}</dt><dd><code>{shortIdentity(record.attemptId)}</code></dd></div>
                <div><dt>{copy('automation.identity.branch_id')}</dt><dd><code>{shortIdentity(record.branchId)}</code></dd></div>
                <div><dt>{copy('automation.identity.parent_branch')}</dt><dd><code>{shortIdentity(record.parentBranchId)}</code></dd></div>
                <div><dt>{copy('automation.identity.operation')}</dt><dd>{copy(`automation.branch.operation.${record.branchOperation}`)}</dd></div>
                <div><dt>{copy('automation.identity.branch_nonce')}</dt><dd>{record.branchNonce}</dd></div>
                <div><dt>{copy('automation.attempts.events')}</dt><dd>{record.events.length}</dd></div>
                <div><dt>{copy('automation.attempts.criterion')}</dt><dd>{record.criterion ? copy(`automation.criterion.${record.criterion.status}`) : copy('automation.criterion.unassigned')}</dd></div>
                <div><dt>{copy('automation.attempts.first_consequence')}</dt><dd>{record.firstConsequenceOrdinal ?? copy('automation.attempts.none')}</dd></div>
              </dl>
              <p>{marginReading(record)}</p>
              <footer>
                <span>{copy('contract.ladder.generator')} <code>{shortIdentity(record.generatorHash)}</code></span>
                <span>{copy('contract.ladder.assembly')} <code>{shortIdentity(record.assemblyHash)}</code></span>
                <span>{copy('automation.attempts.embodied')} <code>{shortIdentity(record.closingEmbodiedHash)}</code></span>
              </footer>
            </li>
          ))}
        </ol>
      )}
      {selected ? (
        <section className="automation-attempt-comparison">
          <header>
            <span>{copy('automation.comparison.title')}</span>
            <strong>{shortIdentity(selected.branchId)} / {shortIdentity(currentIdentity?.branchId)}</strong>
          </header>
          <dl>
            <div>
              <dt>{copy('contract.ladder.generator')}</dt>
              <dd>{copy(selected.generatorHash === currentIdentity?.generatorHash
                ? 'automation.comparison.same'
                : 'automation.comparison.changed')}</dd>
            </div>
            <div>
              <dt>{copy('contract.ladder.assembly')}</dt>
              <dd>{copy(selected.assemblyHash === currentIdentity?.assemblyHash
                ? 'automation.comparison.same'
                : 'automation.comparison.changed')}</dd>
            </div>
            <div>
              <dt>{copy('automation.attempts.criterion')}</dt>
              <dd>{selected.criterion ? copy(`automation.criterion.${selected.criterion.status}`) : copy('automation.criterion.unassigned')} / {currentCriterion ? copy(`automation.criterion.${currentCriterion.status}`) : copy('automation.criterion.unassigned')}</dd>
            </div>
            <div>
              <dt>{copy('automation.comparison.lineage')}</dt>
              <dd>{copy(selected.branchId === currentIdentity?.parentBranchId
                ? 'automation.comparison.direct_parent'
                : 'automation.comparison.retained_branch')}</dd>
            </div>
          </dl>
          <div className="automation-comparison-diff">
            <span>{copy('automation.comparison.first_difference')}</span>
            {difference ? (
              <strong>
                {copy(`automation.event.${difference.retained?.event.kind ?? 'none'}`)} {difference.retained?.step ?? '-'}
                {' / '}
                {copy(`automation.event.${difference.current?.event.kind ?? 'none'}`)} {difference.current?.step ?? '-'}
              </strong>
            ) : <strong>{copy('automation.comparison.no_observed_difference')}</strong>}
          </div>
          {selected.generatorDiff ? (
            <p>
              {copy('automation.comparison.recorded_diff')} {' '}
              {selected.generatorDiff.policyChanged ? copy('automation.comparison.policy_changed') : copy('automation.comparison.policy_same')} {' · '}
              {selected.generatorDiff.topologyChanged ? copy('automation.comparison.topology_changed') : copy('automation.comparison.topology_same')} {' · '}
              {copy('automation.comparison.route_defaults')} {selected.generatorDiff.routeDefaultsChanged.length}
            </p>
          ) : null}
          <p>{copy('automation.comparison.observed_only')}</p>
        </section>
      ) : null}
    </details>
  );
}

function cloneAssemblyDraft(draft: EngineeringAssemblyDraft): EngineeringAssemblyDraft {
  return {
    ...draft,
    components: draft.components.map((component) => ({
      ...component,
      pos: { ...component.pos },
    })),
    currents: draft.currents.map((current) => ({ ...current })),
    forms: draft.forms.map((form) => ({ ...form })),
    materials: draft.materials.map((material) => ({
      ...material,
      pos: { ...material.pos },
    })),
    physical_compartment: {
      ...draft.physical_compartment,
      members: [...draft.physical_compartment.members],
    },
  };
}

function AssemblyUnitInput({
  label,
  value,
  floor,
  ceiling,
  onChange,
}: {
  label: string;
  value: number;
  floor: number;
  ceiling: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="automation-assembly-input">
      <span>{copy(label)}</span>
      <input
        type="number"
        min={floor / WHOLE}
        max={ceiling / WHOLE}
        step="0.25"
        value={fixed(value)}
        onChange={(event) => onChange(rawUnits(event.target.value, floor, ceiling))}
      />
    </label>
  );
}

function AssemblyIntegerInput({
  label,
  value,
  floor,
  ceiling,
  onChange,
}: {
  label: string;
  value: number;
  floor: number;
  ceiling: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="automation-assembly-input">
      <span>{copy(label)}</span>
      <input
        type="number"
        min={floor}
        max={ceiling}
        step="1"
        value={value}
        onChange={(event) => onChange(integer(event.target.value, floor, ceiling))}
      />
    </label>
  );
}

function assemblyDiffValue(change: EngineeringAssemblyDiffChange, side: 'before' | 'after'): string {
  const value = change[side] as Record<string, unknown>;
  const position = value.pos as { x?: unknown; y?: unknown } | undefined;
  if (change.kind === 'component') {
    return [
      `${copy('automation.assembly.layer')} ${String(value.layer)}`,
      `${copy('automation.assembly.position_x')} ${fixed(Number(position?.x ?? 0))}`,
      `${copy('automation.assembly.position_y')} ${fixed(Number(position?.y ?? 0))}`,
      `${copy('automation.assembly.charge')} ${fixed(Number(value.q ?? 0))}`,
      copy(value.open ? 'automation.assembly.interface_open' : 'automation.assembly.interface_closed'),
    ].join(' / ');
  }
  if (change.kind === 'current') {
    return [
      copy(value.active ? 'automation.assembly.current_active' : 'automation.assembly.current_inactive'),
      `${copy('automation.assembly.phase')} ${String(value.phase)}`,
    ].join(' / ');
  }
  if (change.kind === 'form') {
    return [
      `${copy('automation.assembly.reserve')} ${fixed(Number(value.reserve ?? 0))}`,
      value.junction_blanks === null
        ? copy('automation.assembly.blanks_unavailable')
        : `${copy('automation.assembly.blanks')} ${String(value.junction_blanks)}`,
    ].join(' / ');
  }
  if (change.kind === 'material') {
    return [
      `${copy('automation.assembly.amount')} ${String(value.amount)}`,
      `${copy('automation.assembly.layer')} ${String(value.layer)}`,
      `${copy('automation.assembly.position_x')} ${fixed(Number(position?.x ?? 0))}`,
      `${copy('automation.assembly.position_y')} ${fixed(Number(position?.y ?? 0))}`,
    ].join(' / ');
  }
  const members = Array.isArray(value.members) ? value.members.join(', ') : copy('automation.assembly.none');
  return [
    `${copy('automation.assembly.members')} ${members}`,
    `${copy('automation.assembly.leakage')} ${(Number(value.leak_per_exposed_contact_per_step ?? 0) * 100 / WHOLE).toFixed(2)}${copy('unit.percent')}`,
  ].join(' / ');
}

function assemblyRefusalKey(
  answer: ResponseEnvelope,
  fallback: string,
): string {
  if (answer.ok) return fallback;
  const field = answer.error.detail
    && typeof answer.error.detail.field === 'string'
    ? answer.error.detail.field
    : null;
  const known: Record<string, string> = {
    assembly_draft: 'automation.assembly.refusal.address_set',
    assembly_preview: 'automation.assembly.refusal.stale_preview',
    assembly_template: 'automation.assembly.refusal.assembly',
    attempt_branch: 'automation.assembly.refusal.branch',
    authority: 'automation.assembly.refusal.authority',
    components: 'automation.assembly.refusal.components',
    currents: 'automation.assembly.refusal.currents',
    forms: 'automation.assembly.refusal.forms',
    generator_spec: 'automation.assembly.refusal.generator',
    junction_blanks: 'automation.assembly.refusal.junction',
    materials: 'automation.assembly.refusal.materials',
    phase: 'automation.assembly.refusal.phase',
    physical_compartment: 'automation.assembly.refusal.compartment',
    pos: 'automation.assembly.refusal.position',
    run_kind: 'automation.assembly.refusal.run_kind',
    stale_engineering_base: 'automation.assembly.refusal.stale_base',
  };
  return field ? known[field] ?? fallback : fallback;
}

function AssemblyEditor({
  enabled,
  identity,
  onRead,
  onPreview,
  onPreviewChange,
  onCommit,
}: {
  enabled: boolean;
  identity: RunIdentity | null;
  onRead: () => Promise<ResponseEnvelope>;
  onPreview: (draft: EngineeringAssemblyDraft) => Promise<ResponseEnvelope>;
  onPreviewChange: (preview: EngineeringAssemblyPreview | null) => void;
  onCommit: (
    draft: EngineeringAssemblyDraft,
    preview: EngineeringAssemblyPreview,
  ) => Promise<ResponseEnvelope>;
}) {
  const [source, setSource] = useState<EngineeringAssemblyDraftResult | null>(null);
  const [draft, setDraft] = useState<EngineeringAssemblyDraft | null>(null);
  const [preview, setPreview] = useState<EngineeringAssemblyPreview | null>(null);
  const [working, setWorking] = useState(false);
  const [statusKey, setStatusKey] = useState<string | null>(null);
  const sourceBranch = source?.branch_id ?? null;
  const sourceAssembly = source?.assembly_template_hash ?? null;

  const installPreview = useCallback((next: EngineeringAssemblyPreview | null): void => {
    setPreview(next);
    onPreviewChange(next);
  }, [onPreviewChange]);

  useEffect(() => () => onPreviewChange(null), [onPreviewChange]);

  useEffect(() => {
    if (!enabled && preview) installPreview(null);
  }, [enabled, installPreview, preview]);

  useEffect(() => {
    if (
      source
      && (identity?.branchId !== sourceBranch || identity?.assemblyHash !== sourceAssembly)
    ) {
      setSource(null);
      setDraft(null);
      installPreview(null);
      setStatusKey('automation.assembly.status.stale');
    }
  }, [identity?.assemblyHash, identity?.branchId, installPreview, source, sourceAssembly, sourceBranch]);

  const installDraft = (result: EngineeringAssemblyDraftResult): void => {
    setSource(result);
    setDraft(cloneAssemblyDraft(result.assembly_draft));
    installPreview(null);
  };
  const read = async (): Promise<void> => {
    if (working || !enabled) return;
    setWorking(true);
    setStatusKey('automation.assembly.status.loading');
    const answer = await onRead();
    setWorking(false);
    if (!answer.ok) {
      setStatusKey(assemblyRefusalKey(answer, 'automation.assembly.status.read_refused'));
      return;
    }
    installDraft(answer.body as EngineeringAssemblyDraftResult);
    setStatusKey('automation.assembly.status.ready');
  };
  const revise = (next: EngineeringAssemblyDraft): void => {
    setDraft(next);
    installPreview(null);
    setStatusKey('automation.assembly.status.changed');
  };
  const requestPreview = async (): Promise<void> => {
    if (!draft || working || !enabled) return;
    setWorking(true);
    setStatusKey('automation.assembly.status.previewing');
    const answer = await onPreview(cloneAssemblyDraft(draft));
    setWorking(false);
    if (!answer.ok) {
      installPreview(null);
      setStatusKey(assemblyRefusalKey(answer, 'automation.assembly.status.preview_refused'));
      return;
    }
    const accepted = answer.body as EngineeringAssemblyPreview;
    setDraft(cloneAssemblyDraft(accepted.candidate_draft));
    installPreview(accepted);
    setStatusKey(accepted.diff.definition.changes.length > 0
      ? 'automation.assembly.status.preview_ready'
      : 'automation.assembly.status.no_changes');
  };
  const commit = async (): Promise<void> => {
    if (!draft || !preview || working || !enabled || preview.diff.definition.changes.length === 0) return;
    setWorking(true);
    setStatusKey('automation.assembly.status.committing');
    const answer = await onCommit(cloneAssemblyDraft(draft), preview);
    if (!answer.ok) {
      setWorking(false);
      installPreview(null);
      setStatusKey(assemblyRefusalKey(answer, 'automation.assembly.status.commit_refused'));
      return;
    }
    const persistenceState = (answer.body as { persistence_state?: string }).persistence_state;
    installPreview(null);
    const refreshed = await onRead();
    setWorking(false);
    if (refreshed.ok) installDraft(refreshed.body as EngineeringAssemblyDraftResult);
    else {
      setSource(null);
      setDraft(null);
      installPreview(null);
    }
    setStatusKey(persistenceState === 'recovery_required'
      ? 'automation.assembly.status.recovery_required'
      : 'automation.assembly.status.committed');
  };
  const dirty = Boolean(
    source
    && draft
    && JSON.stringify(source.assembly_draft) !== JSON.stringify(draft),
  );

  return (
    <section className="automation-assembly-editor" data-preview={preview ? 'accepted' : 'none'}>
      <header>
        <div>
          <span>{copy('automation.assembly.eyebrow')}</span>
          <strong>{copy('automation.assembly.title')}</strong>
        </div>
        <button type="button" onClick={() => { void read(); }} disabled={!enabled || working}>
          {copy(source ? 'automation.assembly.reload' : 'automation.assembly.load')}
        </button>
      </header>
      <p>{copy('automation.assembly.boundary')}</p>
      {!enabled ? <p role="status">{copy('automation.assembly.design_required')}</p> : null}
      {statusKey ? <p className="automation-assembly-status" role="status">{copy(statusKey)}</p> : null}
      {source && draft ? (
        <>
          <dl className="automation-assembly-authority">
            <div><dt>{copy('automation.identity.branch')}</dt><dd><code>{shortIdentity(source.branch_id)}</code></dd></div>
            <div><dt>{copy('contract.ladder.generator')}</dt><dd><code>{shortIdentity(source.generator_spec_hash)}</code></dd></div>
            <div><dt>{copy('contract.ladder.assembly')}</dt><dd><code>{shortIdentity(source.assembly_template_hash)}</code></dd></div>
            <div><dt>{copy('automation.blueprint.authority')}</dt><dd>{copy('automation.assembly.authority_committed')}</dd></div>
          </dl>

          <details open className="automation-assembly-register">
            <summary>
              <span>{copy('automation.assembly.components')}</span>
              <strong>{draft.components.length}</strong>
            </summary>
            <p>{copy('automation.assembly.components_explanation')}</p>
            <div className="automation-assembly-rows">
              {draft.components.map((component, index) => (
                <fieldset key={component.node}>
                  <legend>{copy('automation.assembly.component')} {component.node}</legend>
                  <div className="automation-assembly-fields">
                    <AssemblyIntegerInput
                      label="automation.assembly.layer"
                      value={component.layer}
                      floor={0}
                      ceiling={7}
                      onChange={(layer) => {
                        const components = [...draft.components];
                        components[index] = { ...component, layer };
                        revise({ ...draft, components });
                      }}
                    />
                    <AssemblyUnitInput
                      label="automation.assembly.position_x"
                      value={component.pos.x}
                      floor={0}
                      ceiling={(4096 * WHOLE) - 1}
                      onChange={(x) => {
                        const components = [...draft.components];
                        components[index] = { ...component, pos: { ...component.pos, x } };
                        revise({ ...draft, components });
                      }}
                    />
                    <AssemblyUnitInput
                      label="automation.assembly.position_y"
                      value={component.pos.y}
                      floor={0}
                      ceiling={(4096 * WHOLE) - 1}
                      onChange={(y) => {
                        const components = [...draft.components];
                        components[index] = { ...component, pos: { ...component.pos, y } };
                        revise({ ...draft, components });
                      }}
                    />
                    <AssemblyUnitInput
                      label="automation.assembly.charge"
                      value={component.q}
                      floor={0}
                      ceiling={4096 * WHOLE}
                      onChange={(q) => {
                        const components = [...draft.components];
                        components[index] = { ...component, q };
                        revise({ ...draft, components });
                      }}
                    />
                    <label className="automation-toggle automation-assembly-toggle">
                      <input
                        type="checkbox"
                        checked={component.open}
                        onChange={(event) => {
                          const components = [...draft.components];
                          components[index] = { ...component, open: event.target.checked };
                          revise({ ...draft, components });
                        }}
                      />
                      <span>{copy(component.open
                        ? 'automation.assembly.interface_open'
                        : 'automation.assembly.interface_closed')}</span>
                    </label>
                  </div>
                </fieldset>
              ))}
            </div>
          </details>

          <details className="automation-assembly-register">
            <summary>
              <span>{copy('automation.assembly.forms')}</span>
              <strong>{draft.forms.length}</strong>
            </summary>
            <p>{copy('automation.assembly.forms_explanation')}</p>
            <div className="automation-assembly-rows">
              {draft.forms.map((form, index) => (
                <fieldset key={form.node}>
                  <legend>{copy('automation.assembly.form')} {form.node}</legend>
                  <div className="automation-assembly-fields">
                    <AssemblyUnitInput
                      label="automation.assembly.reserve"
                      value={form.reserve}
                      floor={0}
                      ceiling={4096 * WHOLE}
                      onChange={(reserve) => {
                        const forms = [...draft.forms];
                        forms[index] = { ...form, reserve };
                        revise({ ...draft, forms });
                      }}
                    />
                    {form.junction_blanks === null ? (
                      <span className="automation-assembly-locked">{copy('automation.assembly.blanks_unavailable')}</span>
                    ) : (
                      <AssemblyIntegerInput
                        label="automation.assembly.blanks"
                        value={form.junction_blanks}
                        floor={0}
                        ceiling={255}
                        onChange={(junction_blanks) => {
                          const forms = [...draft.forms];
                          forms[index] = { ...form, junction_blanks };
                          revise({ ...draft, forms });
                        }}
                      />
                    )}
                  </div>
                </fieldset>
              ))}
            </div>
          </details>

          <details className="automation-assembly-register">
            <summary>
              <span>{copy('automation.assembly.materials')}</span>
              <strong>{draft.materials.length}</strong>
            </summary>
            <p>{copy('automation.assembly.materials_explanation')}</p>
            <div className="automation-assembly-rows">
              {draft.materials.map((material, index) => (
                <fieldset key={material.material}>
                  <legend>{copy('automation.assembly.material')} {material.material}</legend>
                  <div className="automation-assembly-fields">
                    <AssemblyIntegerInput
                      label="automation.assembly.amount"
                      value={material.amount}
                      floor={0}
                      ceiling={65_535}
                      onChange={(amount) => {
                        const materials = [...draft.materials];
                        materials[index] = { ...material, amount };
                        revise({ ...draft, materials });
                      }}
                    />
                    <AssemblyIntegerInput
                      label="automation.assembly.layer"
                      value={material.layer}
                      floor={0}
                      ceiling={7}
                      onChange={(layer) => {
                        const materials = [...draft.materials];
                        materials[index] = { ...material, layer };
                        revise({ ...draft, materials });
                      }}
                    />
                    <AssemblyUnitInput
                      label="automation.assembly.position_x"
                      value={material.pos.x}
                      floor={0}
                      ceiling={(4096 * WHOLE) - 1}
                      onChange={(x) => {
                        const materials = [...draft.materials];
                        materials[index] = { ...material, pos: { ...material.pos, x } };
                        revise({ ...draft, materials });
                      }}
                    />
                    <AssemblyUnitInput
                      label="automation.assembly.position_y"
                      value={material.pos.y}
                      floor={0}
                      ceiling={(4096 * WHOLE) - 1}
                      onChange={(y) => {
                        const materials = [...draft.materials];
                        materials[index] = { ...material, pos: { ...material.pos, y } };
                        revise({ ...draft, materials });
                      }}
                    />
                  </div>
                </fieldset>
              ))}
            </div>
          </details>

          <details className="automation-assembly-register">
            <summary>
              <span>{copy('automation.assembly.currents')}</span>
              <strong>{draft.currents.length}</strong>
            </summary>
            <p>{copy('automation.assembly.currents_explanation')}</p>
            <div className="automation-assembly-rows">
              {draft.currents.map((current, index) => (
                <fieldset key={current.current}>
                  <legend>{copy('automation.assembly.current')} {current.current}</legend>
                  <div className="automation-assembly-fields">
                    <AssemblyIntegerInput
                      label="automation.assembly.phase"
                      value={current.phase}
                      floor={0}
                      ceiling={65_535}
                      onChange={(phase) => {
                        const currents = [...draft.currents];
                        currents[index] = { ...current, phase };
                        revise({ ...draft, currents });
                      }}
                    />
                    <label className="automation-toggle automation-assembly-toggle">
                      <input
                        type="checkbox"
                        checked={current.active}
                        onChange={(event) => {
                          const currents = [...draft.currents];
                          currents[index] = { ...current, active: event.target.checked };
                          revise({ ...draft, currents });
                        }}
                      />
                      <span>{copy(current.active
                        ? 'automation.assembly.current_active'
                        : 'automation.assembly.current_inactive')}</span>
                    </label>
                  </div>
                </fieldset>
              ))}
            </div>
          </details>

          <details className="automation-assembly-register">
            <summary>
              <span>{copy('automation.assembly.compartment')}</span>
              <strong>{draft.physical_compartment.members.length}</strong>
            </summary>
            <p>{copy('automation.assembly.compartment_explanation')}</p>
            <div className="automation-assembly-members">
              {draft.components.map((component) => {
                const checked = draft.physical_compartment.members.includes(component.node);
                return (
                  <label key={component.node} className="automation-toggle">
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={(event) => {
                        const members = event.target.checked
                          ? [...draft.physical_compartment.members, component.node]
                          : draft.physical_compartment.members.filter((node) => node !== component.node);
                        members.sort((left, right) => left - right);
                        revise({
                          ...draft,
                          physical_compartment: { ...draft.physical_compartment, members },
                        });
                      }}
                    />
                    <span>{copy('automation.assembly.component')} {component.node}</span>
                  </label>
                );
              })}
            </div>
            <label className="automation-assembly-leakage">
              <span>{copy('automation.assembly.leakage')}</span>
              <input
                type="range"
                min="0"
                max={WHOLE}
                step="256"
                value={draft.physical_compartment.leak_per_exposed_contact_per_step}
                onChange={(event) => revise({
                  ...draft,
                  physical_compartment: {
                    ...draft.physical_compartment,
                    leak_per_exposed_contact_per_step: integer(event.target.value, 0, WHOLE),
                  },
                })}
              />
              <output>
                {(draft.physical_compartment.leak_per_exposed_contact_per_step * 100 / WHOLE).toFixed(2)}
                {copy('unit.percent')}
              </output>
            </label>
            <p>{copy('automation.assembly.attachment_boundary')}</p>
          </details>

          <section className="automation-assembly-diff" aria-label={copy('automation.assembly.diff_title')}>
            <header>
              <div>
                <span>{copy('automation.assembly.diff_title')}</span>
                <strong>{preview?.diff.definition.changes.length ?? 0}</strong>
              </div>
              <span>{copy(preview
                ? 'automation.assembly.preview_noncausal'
                : 'automation.assembly.preview_absent')}</span>
            </header>
            {preview ? (
              <p className="automation-assembly-compatible">
                {copy('automation.assembly.compatible')}
              </p>
            ) : null}
            {preview?.warnings.length ? (
              <ul className="automation-assembly-warnings">
                {preview.warnings.map((warning, index) => (
                  <li key={`${warning.code}:${warning.address ?? index}`}>
                    {copy(`automation.assembly.warning.${warning.code}`)}
                    {warning.address ? <code>{warning.address}</code> : null}
                  </li>
                ))}
              </ul>
            ) : null}
            {preview && preview.diff.definition.changes.length > 0 ? (
              <ol>
                {preview.diff.definition.changes.map((change) => (
                  <li key={change.address}>
                    <header>
                      <strong>{change.address}</strong>
                      <span>{copy(`automation.assembly.kind.${change.kind}`)}</span>
                    </header>
                    <div><span>{copy('automation.assembly.before')}</span><p>{assemblyDiffValue(change, 'before')}</p></div>
                    <div><span>{copy('automation.assembly.after')}</span><p>{assemblyDiffValue(change, 'after')}</p></div>
                  </li>
                ))}
              </ol>
            ) : <p>{copy(preview ? 'automation.assembly.diff_empty' : 'automation.assembly.diff_prompt')}</p>}
          </section>

          <footer className="automation-assembly-actions">
            <button
              type="button"
              onClick={() => { void requestPreview(); }}
              disabled={!enabled || working || !dirty}
            >
              {copy('automation.assembly.preview')}
            </button>
            <button
              type="button"
              onClick={() => { void commit(); }}
              disabled={!enabled || working || !preview || preview.diff.definition.changes.length === 0}
            >
              {copy('automation.assembly.commit')}
            </button>
          </footer>
        </>
      ) : null}
    </section>
  );
}

function TransitionOpeningReadings({ draft }: { draft: EngineeringAssemblyDraft }) {
  const compartment = draft.physical_compartment;
  return (
    <section className="automation-transition-opening">
      <header>
        <h4>{copy('automation.transition.opening.title')}</h4>
        <span>{copy('automation.transition.opening.step_zero')}</span>
      </header>
      <p>{copy('automation.transition.opening.explanation')}</p>
      <dl className="automation-transition-opening-overview">
        <div><dt>{copy('automation.assembly.components')}</dt><dd>{draft.components.length}</dd></div>
        <div><dt>{copy('automation.assembly.forms')}</dt><dd>{draft.forms.length}</dd></div>
        <div><dt>{copy('automation.assembly.materials')}</dt><dd>{draft.materials.length}</dd></div>
        <div><dt>{copy('automation.assembly.currents')}</dt><dd>{draft.currents.length}</dd></div>
        <div><dt>{copy('automation.assembly.members')}</dt><dd>{compartment.members.length}</dd></div>
      </dl>

      <details open className="automation-transition-opening-register">
        <summary>
          <span>{copy('automation.assembly.components')}</span>
          <strong>{draft.components.length}</strong>
        </summary>
        <div className="automation-transition-opening-table">
          <table>
            <thead>
              <tr>
                <th scope="col">{copy('automation.assembly.component')}</th>
                <th scope="col">{copy('automation.assembly.layer')}</th>
                <th scope="col">{copy('automation.assembly.position_x')}</th>
                <th scope="col">{copy('automation.assembly.position_y')}</th>
                <th scope="col">{copy('automation.assembly.charge')}</th>
                <th scope="col">{copy('automation.transition.opening.interface')}</th>
              </tr>
            </thead>
            <tbody>
              {draft.components.map((component) => (
                <tr key={component.node}>
                  <th scope="row">{component.node}</th>
                  <td>{component.layer}</td>
                  <td>{fixed(component.pos.x)}</td>
                  <td>{fixed(component.pos.y)}</td>
                  <td>{fixed(component.q)}</td>
                  <td>{copy(component.open
                    ? 'automation.assembly.interface_open'
                    : 'automation.assembly.interface_closed')}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </details>

      <details className="automation-transition-opening-register">
        <summary>
          <span>{copy('automation.assembly.forms')}</span>
          <strong>{draft.forms.length}</strong>
        </summary>
        <div className="automation-transition-opening-table">
          <table>
            <thead>
              <tr>
                <th scope="col">{copy('automation.assembly.form')}</th>
                <th scope="col">{copy('automation.assembly.reserve')}</th>
                <th scope="col">{copy('automation.assembly.blanks')}</th>
              </tr>
            </thead>
            <tbody>
              {draft.forms.map((form) => (
                <tr key={form.node}>
                  <th scope="row">{form.node}</th>
                  <td>{fixed(form.reserve)}</td>
                  <td>{form.junction_blanks ?? copy('automation.assembly.blanks_unavailable')}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </details>

      <details className="automation-transition-opening-register">
        <summary>
          <span>{copy('automation.assembly.materials')}</span>
          <strong>{draft.materials.length}</strong>
        </summary>
        <div className="automation-transition-opening-table">
          <table>
            <thead>
              <tr>
                <th scope="col">{copy('automation.assembly.material')}</th>
                <th scope="col">{copy('automation.assembly.amount')}</th>
                <th scope="col">{copy('automation.assembly.layer')}</th>
                <th scope="col">{copy('automation.assembly.position_x')}</th>
                <th scope="col">{copy('automation.assembly.position_y')}</th>
              </tr>
            </thead>
            <tbody>
              {draft.materials.map((material) => (
                <tr key={material.material}>
                  <th scope="row">{material.material}</th>
                  <td>{material.amount}</td>
                  <td>{material.layer}</td>
                  <td>{fixed(material.pos.x)}</td>
                  <td>{fixed(material.pos.y)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </details>

      <details className="automation-transition-opening-register">
        <summary>
          <span>{copy('automation.assembly.currents')}</span>
          <strong>{draft.currents.length}</strong>
        </summary>
        <div className="automation-transition-opening-table">
          <table>
            <thead>
              <tr>
                <th scope="col">{copy('automation.assembly.current')}</th>
                <th scope="col">{copy('automation.transition.opening.current_stance')}</th>
                <th scope="col">{copy('automation.assembly.phase')}</th>
              </tr>
            </thead>
            <tbody>
              {draft.currents.map((current) => (
                <tr key={current.current}>
                  <th scope="row">{current.current}</th>
                  <td>{copy(current.active
                    ? 'automation.assembly.current_active'
                    : 'automation.assembly.current_inactive')}</td>
                  <td>{current.phase}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </details>

      <details className="automation-transition-opening-register">
        <summary>
          <span>{copy('automation.assembly.compartment')}</span>
          <strong>{compartment.members.length}</strong>
        </summary>
        <dl className="automation-transition-opening-compartment">
          <div>
            <dt>{copy('automation.assembly.members')}</dt>
            <dd>
              {compartment.members.length > 0
                ? compartment.members.map((member) => (
                    <code key={member}>{copy('automation.assembly.component')} {member}</code>
                  ))
                : copy('automation.assembly.none')}
            </dd>
          </div>
          <div>
            <dt>{copy('automation.assembly.leakage')}</dt>
            <dd>
              {(compartment.leak_per_exposed_contact_per_step * 100 / WHOLE).toFixed(2)}
              {copy('unit.percent')}
            </dd>
          </div>
        </dl>
      </details>
    </section>
  );
}

function EngineeringMemory({
  records,
  generatorSources,
  generatorSourceReadError,
  migration,
  recoveries,
  identity,
  enabled,
  onCapture,
  onReadAssembly,
  onPreviewAssembly,
  onAssemblyPreviewChange,
  onCommitAssembly,
  onPreviewTransition,
  onTransitionPreviewChange,
  onCommitTransition,
}: {
  records: readonly EngineeringBlueprintEntry[];
  generatorSources: readonly EngineeringGeneratorSourceEntry[];
  generatorSourceReadError: boolean;
  migration: EngineeringMigrationJournal | null;
  recoveries: readonly EngineeringOperationRecovery[];
  identity: RunIdentity | null;
  enabled: boolean;
  onCapture: (name: string) => Promise<ResponseEnvelope>;
  onReadAssembly: () => Promise<ResponseEnvelope>;
  onPreviewAssembly: (draft: EngineeringAssemblyDraft) => Promise<ResponseEnvelope>;
  onAssemblyPreviewChange: (preview: EngineeringAssemblyPreview | null) => void;
  onCommitAssembly: (
    draft: EngineeringAssemblyDraft,
    preview: EngineeringAssemblyPreview,
  ) => Promise<ResponseEnvelope>;
  onPreviewTransition: (
    operation: EngineeringTransitionKind,
    entry: EngineeringGeneratorSourceEntry | null,
  ) => Promise<ResponseEnvelope>;
  onTransitionPreviewChange: (preview: EngineeringRunTransitionPreview | null) => void;
  onCommitTransition: (
    preview: EngineeringRunTransitionPreview,
  ) => Promise<ResponseEnvelope>;
}) {
  const [name, setName] = useState('');
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [pendingTransition, setPendingTransition] = useState<{
    entry: EngineeringGeneratorSourceEntry | null;
    operation: EngineeringTransitionKind;
    preview: EngineeringRunTransitionPreview | null;
    refusal: EngineeringTransitionRefused | null;
  } | null>(null);
  const capture = async (): Promise<void> => {
    if (saving || !enabled || !identity?.assemblyExact || !identity.branchId) return;
    setSaving(true);
    setStatus(copy('automation.blueprint.capturing'));
    const answer = await onCapture(name);
    setSaving(false);
    if (!answer.ok) {
      setStatus(copy('automation.blueprint.refused'));
      return;
    }
    setName('');
    setStatus(copy('automation.blueprint.retained'));
  };
  const openTransition = async (
    operation: EngineeringTransitionKind,
    entry: EngineeringGeneratorSourceEntry | null,
  ): Promise<void> => {
    if (saving || !enabled) return;
    onTransitionPreviewChange(null);
    setPendingTransition({ entry, operation, preview: null, refusal: null });
    setSaving(true);
    setStatus(copy(`automation.transition.status.previewing.${operation}`));
    const answer = await onPreviewTransition(operation, entry);
    setSaving(false);
    if (!answer.ok) {
      onTransitionPreviewChange(null);
      setStatus(copy('automation.transition.status.transport_refused'));
      return;
    }
    const body = answer.body as EngineeringTransitionPreviewAccepted | EngineeringTransitionRefused;
    if (body.status === 'refused') {
      onTransitionPreviewChange(null);
      setPendingTransition({ entry, operation, preview: null, refusal: body });
      setStatus(copy(`automation.transition.refusal.${body.code}`));
      return;
    }
    setPendingTransition({ entry, operation, preview: body.preview, refusal: null });
    onTransitionPreviewChange(body.preview);
    setStatus(copy(body.preview.definition.commit_allowed
      ? `automation.transition.status.ready.${operation}`
      : 'automation.transition.status.incompatible'));
  };
  const applyTransition = async (): Promise<void> => {
    if (!pendingTransition?.preview || saving || !enabled) return;
    if (!pendingTransition.preview.definition.commit_allowed) {
      setStatus(copy('automation.transition.status.commit_blocked'));
      return;
    }
    setSaving(true);
    setStatus(copy(`automation.transition.status.committing.${pendingTransition.operation}`));
    const answer = await onCommitTransition(pendingTransition.preview);
    setSaving(false);
    if (!answer.ok) {
      setStatus(copy('automation.transition.status.transport_refused'));
      return;
    }
    const body = answer.body as { code?: string; status?: string };
    if (body.status === 'refused') {
      setStatus(copy(`automation.transition.refusal.${body.code ?? 'stale_preview'}`));
      return;
    }
    const operation = pendingTransition.operation;
    setPendingTransition(null);
    setStatus(copy(`automation.transition.status.committed.${operation}`));
  };
  return (
    <details className="automation-blueprints">
      <summary>
        <span>{copy('automation.blueprint.title')}</span>
        <strong>{records.length}</strong>
      </summary>
      <p>{copy('automation.blueprint.boundary')}</p>
      {migration ? (
        <section className="automation-migration" data-state={migration.state}>
          <header>
            <strong>{copy('automation.blueprint.migration_title')}</strong>
            <span>{copy(`automation.blueprint.migration_state.${migration.state}`)}</span>
          </header>
          <dl>
            <div><dt>{copy('automation.blueprint.migration_v1')}</dt><dd>{migration.preservedV1Count}</dd></div>
            <div><dt>{copy('automation.blueprint.migration_v2')}</dt><dd>{migration.currentV2Count}</dd></div>
            <div><dt>{copy('automation.blueprint.migration_unsupported')}</dt><dd>{migration.unsupportedCount}</dd></div>
            <div><dt>{copy('automation.blueprint.migration_conflicts')}</dt><dd>{migration.conflictCount}</dd></div>
            <div><dt>{copy('automation.blueprint.migration_unavailable')}</dt><dd>{migration.unavailableRelationshipCount}</dd></div>
          </dl>
          <p>{copy(`automation.blueprint.migration_explanation.${migration.state}`)}</p>
          {migration.conflictIds.length
            + migration.unsupportedIds.length
            + migration.unavailableRelationshipIds.length > 0 ? (
              <details>
                <summary>{copy('automation.blueprint.migration_details')}</summary>
                <ul>
                  {[
                    ...migration.conflictIds,
                    ...migration.unsupportedIds,
                    ...migration.unavailableRelationshipIds,
                  ].map((id) => <li key={id}><code>{id}</code></li>)}
                </ul>
              </details>
            ) : null}
        </section>
      ) : null}
      {recoveries.length > 0 ? (
        <section
          className="automation-operation-recovery"
          data-state={recoveries.some((outcome) => outcome.status === 'manual_recovery')
            ? 'manual_recovery'
            : 'resolved'}
        >
          <header>
            <strong>{copy('automation.operation_recovery.title')}</strong>
            <span>{recoveries.length}</span>
          </header>
          <p>{copy('automation.operation_recovery.explanation')}</p>
          <ol>
            {recoveries.slice(-4).reverse().map((outcome) => (
              <li key={outcome.previewId} data-status={outcome.status}>
                <div>
                  <strong>{copy(`automation.operation_recovery.status.${outcome.status}`)}</strong>
                  <span>{copy(`automation.operation_recovery.operation.${outcome.operation}`)}</span>
                  <code>{shortIdentity(outcome.operationId ?? outcome.previewId)}</code>
                </div>
                {outcome.error ? <p>{copy('automation.operation_recovery.error')} <code>{outcome.error}</code></p> : null}
              </li>
            ))}
          </ol>
        </section>
      ) : null}
      <AssemblyEditor
        enabled={enabled && Boolean(identity?.assemblyExact && identity.branchId)}
        identity={identity}
        onRead={onReadAssembly}
        onPreview={onPreviewAssembly}
        onPreviewChange={onAssemblyPreviewChange}
        onCommit={onCommitAssembly}
      />
      <div className="automation-blueprint-capture">
        <label>
          <span>{copy('automation.blueprint.name')}</span>
          <input
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder={copy('automation.blueprint.name_placeholder')}
            maxLength={80}
          />
        </label>
        <button
          type="button"
          onClick={() => { void capture(); }}
          disabled={saving || !enabled || !identity?.assemblyExact || !identity.branchId}
        >
          {copy('automation.blueprint.capture')}
        </button>
      </div>
      {!identity?.assemblyExact ? <p role="status">{copy('automation.blueprint.exact_required')}</p> : null}
      {status ? <p role="status">{status}</p> : null}
      <div className="automation-blueprint-global-actions">
        <button
          type="button"
          disabled={!enabled || saving}
          onClick={() => { void openTransition('restart_assembly', null); }}
        >
          {copy('automation.transition.command.restart_assembly')}
        </button>
        <button
          type="button"
          disabled={!enabled || saving}
          onClick={() => { void openTransition('full_contract_reset', null); }}
        >
          {copy('automation.transition.command.full_contract_reset')}
        </button>
      </div>
      <section className="automation-generator-sources">
        <header>
          <div>
            <span>{copy('automation.transition.sources.eyebrow')}</span>
            <h4>{copy('automation.transition.sources.title')}</h4>
          </div>
          <strong>{generatorSources.length}</strong>
        </header>
        <p>{copy('automation.transition.sources.explanation')}</p>
        {generatorSourceReadError ? (
          <p role="status">{copy('automation.transition.sources.read_error')}</p>
        ) : generatorSources.length === 0 ? (
          <p>{copy('automation.transition.sources.empty')}</p>
        ) : (
          <ol>
            {generatorSources.map((source) => (
              <li
                key={source.id}
                data-availability={source.availability}
                data-source-kind={source.kind}
              >
                <header>
                  <div>
                    <span>{copy(`automation.transition.sources.kind.${source.kind}`)}</span>
                    <strong>{source.name ?? copy(`automation.transition.sources.kind.${source.kind}`)}</strong>
                  </div>
                  <span>{copy(`automation.transition.sources.availability.${source.availability}`)}</span>
                </header>
                <dl>
                  <div><dt>{copy('automation.transition.sources.source_id')}</dt><dd><code>{shortIdentity(source.sourceId)}</code></dd></div>
                  <div><dt>{copy('automation.transition.sources.branch_id')}</dt><dd><code>{shortIdentity(source.branchId)}</code></dd></div>
                  <div><dt>{copy('automation.transition.sources.distance')}</dt><dd>{source.ancestorDistance}</dd></div>
                  <div><dt>{copy('automation.transition.sources.generator_hash')}</dt><dd><code>{source.generatorHash ? shortIdentity(source.generatorHash) : copy('automation.transition.sources.none')}</code></dd></div>
                  <div><dt>{copy('automation.transition.sources.generator_record')}</dt><dd><code>{source.generatorRecordId ? shortIdentity(source.generatorRecordId) : copy('automation.transition.sources.none')}</code></dd></div>
                  <div><dt>{copy('automation.transition.sources.schema')}</dt><dd>{source.sourceSchema} / {source.generatorSchema ?? copy('automation.transition.sources.none')}</dd></div>
                  {source.branchOperation ? (
                    <div><dt>{copy('automation.identity.operation')}</dt><dd>{copy(`automation.branch.operation.${source.branchOperation}`)}</dd></div>
                  ) : null}
                  {source.resultOutcome ? (
                    <div><dt>{copy('automation.transition.sources.result')}</dt><dd>{copy(`automation.qualification.result.${source.resultOutcome}`)}</dd></div>
                  ) : null}
                </dl>
                <p role="status">{copy(`automation.transition.sources.reason.${source.reason}`)}</p>
                <button
                  type="button"
                  disabled={!enabled || saving || source.availability !== 'available' || !source.generator}
                  onClick={() => { void openTransition('revert_generator', source); }}
                >
                  {copy('automation.transition.command.revert_generator')}
                </button>
              </li>
            ))}
          </ol>
        )}
      </section>
      {records.length === 0 ? <p>{copy('automation.blueprint.empty')}</p> : (
        <ol>
          {records.slice(0, 12).map((entry) => {
            const { metadata, record } = entry;
            const sourceBranchId = record.definition.version === 1
              ? record.definition.branch_id
              : record.definition.source_branch_id;
            const resultCount = record.definition.version === 1
              ? record.definition.linked_result_ids.length
              : record.definition.evidence_links.filter((link) => (
                link.evidence_kind === 'qualification_result'
              )).length;
            const relationshipsAvailable = entry.unavailableRelationships.length === 0;
            return (
            <li
              key={record.blueprint_id}
              data-record-authority={record.definition.version === 1 ? 'legacy-read-only' : 'current'}
            >
              {metadata.thumbnail ? (
                <figure className="automation-blueprint-thumbnail">
                  <img
                    src={metadata.thumbnail.dataUrl}
                    alt={copy('automation.blueprint.thumbnail_alt')}
                    width={metadata.thumbnail.width}
                    height={metadata.thumbnail.height}
                  />
                  <figcaption>
                    {copy('automation.blueprint.thumbnail_source')} {' '}
                    <code>{shortIdentity(metadata.thumbnail.assemblyHash)}</code>
                  </figcaption>
                </figure>
              ) : null}
              <header>
                <strong>{metadata.name}</strong>
                <span>{record.definition.contract_id}</span>
              </header>
              <dl>
                <div><dt>{copy('automation.blueprint.identity')}</dt><dd><code>{shortIdentity(record.blueprint_id)}</code></dd></div>
                <div><dt>{copy('contract.ladder.generator')}</dt><dd><code>{shortIdentity(record.definition.generator_record_id)}</code></dd></div>
                <div><dt>{copy('contract.ladder.assembly')}</dt><dd><code>{shortIdentity(record.definition.assembly_record_id)}</code></dd></div>
                <div><dt>{copy('automation.blueprint.source_branch')}</dt><dd><code>{shortIdentity(sourceBranchId)}</code></dd></div>
                <div><dt>{copy('automation.blueprint.results')}</dt><dd>{resultCount}</dd></div>
                <div><dt>{copy('automation.blueprint.authority')}</dt><dd>{copy(record.definition.version === 1 ? 'automation.blueprint.authority.v1' : 'automation.blueprint.authority.v2')}</dd></div>
              </dl>
              {record.definition.version === 1 ? (
                <p role="status">{copy('automation.blueprint.legacy_read_only')}</p>
              ) : null}
              {!relationshipsAvailable ? (
                <p role="status">
                  {copy('automation.blueprint.relationship_unavailable')} {' '}
                  {entry.unavailableRelationships
                    .map((relationship) => copy(`automation.blueprint.relationship.${relationship}`))
                    .join(', ')}
                </p>
              ) : null}
            </li>
            );
          })}
        </ol>
      )}
      {pendingTransition ? (
        <section
          className="automation-blueprint-reset-confirm"
          data-operation={pendingTransition.operation}
          data-state={pendingTransition.refusal
            ? 'refused'
            : pendingTransition.preview
              ? pendingTransition.preview.definition.commit_allowed ? 'ready' : 'incompatible'
              : 'previewing'}
        >
          <header>
            <div>
              <span>{copy('automation.transition.eyebrow')}</span>
              <h3>{copy(`automation.transition.title.${pendingTransition.operation}`)}</h3>
            </div>
            <strong>{copy(pendingTransition.preview
              ? pendingTransition.preview.definition.commit_allowed
                ? 'automation.transition.preview_noncausal'
                : 'automation.transition.preview_incompatible'
              : 'automation.transition.preview_pending')}</strong>
          </header>
          <p>{copy(`automation.transition.explanation.${pendingTransition.operation}`)}</p>
          {pendingTransition.entry ? (
            <p>
              {copy('automation.transition.selected_source')} {' '}
              <strong>{pendingTransition.entry.name ?? copy(`automation.transition.sources.kind.${pendingTransition.entry.kind}`)}</strong> {' '}
              <code>{shortIdentity(pendingTransition.entry.sourceId)}</code>
            </p>
          ) : null}
          {pendingTransition.refusal ? (
            <p role="status">{copy(`automation.transition.refusal.${pendingTransition.refusal.code}`)}</p>
          ) : null}
          {pendingTransition.preview ? (
            <>
              <dl className="automation-transition-authority">
                <div><dt>{copy('automation.transition.preview_id')}</dt><dd><code>{shortIdentity(pendingTransition.preview.preview_id)}</code></dd></div>
                <div><dt>{copy('automation.transition.source')}</dt><dd>{copy(`automation.transition.source.${pendingTransition.preview.definition.source.kind}`)}</dd></div>
                <div><dt>{copy('automation.transition.current_embodied')}</dt><dd><code>{shortIdentity(pendingTransition.preview.definition.guard.embodied_hash)}</code></dd></div>
                <div><dt>{copy('automation.transition.current_regime')}</dt><dd>{pendingTransition.preview.definition.current_regime_id}</dd></div>
                <div><dt>{copy('automation.transition.current_scenario')}</dt><dd><code>{shortIdentity(pendingTransition.preview.definition.guard.scenario_hash)}</code></dd></div>
                <div><dt>{copy('automation.transition.target_generator')}</dt><dd><code>{shortIdentity(pendingTransition.preview.definition.target_generator_hash)}</code></dd></div>
                <div><dt>{copy('automation.transition.target_assembly')}</dt><dd><code>{shortIdentity(pendingTransition.preview.definition.target_assembly_hash)}</code></dd></div>
                <div><dt>{copy('automation.transition.target_regime')}</dt><dd>{pendingTransition.preview.definition.target_regime_id}</dd></div>
                <div><dt>{copy('automation.transition.target_scenario')}</dt><dd><code>{shortIdentity(pendingTransition.preview.definition.target_scenario_hash)}</code></dd></div>
                <div><dt>{copy('automation.transition.reconstruction')}</dt><dd><code>{shortIdentity(pendingTransition.preview.definition.reconstruction_digest)}</code></dd></div>
              </dl>
              <TransitionOpeningReadings
                draft={pendingTransition.preview.definition.target_assembly_draft}
              />
              <section className="automation-transition-registers">
                <header>
                  <h4>{copy('automation.transition.registers')}</h4>
                  <span>{copy('automation.transition.registers_boundary')}</span>
                </header>
                <ol>
                  {pendingTransition.preview.definition.registers.map((register) => (
                    <li key={register.kind} data-register={register.kind}>
                      <header>
                        <strong>{copy(`automation.transition.register.${register.kind}`)}</strong>
                        <span>
                          {copy(`automation.transition.disposition.${register.before_disposition}`)}
                          {' -> '}
                          {copy(`automation.transition.disposition.${register.after_disposition}`)}
                        </span>
                      </header>
                      <dl>
                        <div>
                          <dt>{copy('automation.transition.register.before_digest')}</dt>
                          <dd><code>{shortIdentity(register.before_digest)}</code></dd>
                        </div>
                        <div>
                          <dt>{copy('automation.transition.register.after_digest')}</dt>
                          <dd><code>{shortIdentity(register.after_digest)}</code></dd>
                        </div>
                      </dl>
                      <details>
                        <summary>
                          {copy('automation.transition.register.addresses')} {register.addresses.length}
                        </summary>
                        {register.addresses.length > 0 ? (
                          <div>{register.addresses.map((address) => <code key={address}>{address}</code>)}</div>
                        ) : <p>{copy('automation.transition.register.no_addresses')}</p>}
                      </details>
                    </li>
                  ))}
                </ol>
              </section>
              <section className="automation-transition-consequences">
                <h4>{copy('automation.transition.consequences')}</h4>
                <ol>
                  {pendingTransition.preview.definition.identities.map((consequence) => (
                    <li key={`${consequence.disposition}:${consequence.kind}:${consequence.identity}`} data-disposition={consequence.disposition}>
                      <strong>{copy(`automation.transition.disposition.${consequence.disposition}`)}</strong>
                      <span>{copy(`automation.transition.identity.${consequence.kind}`)}</span>
                      <code>{shortIdentity(consequence.identity)}</code>
                    </li>
                  ))}
                </ol>
              </section>
              {pendingTransition.preview.definition.compatibility_fields.length > 0 ? (
                <section className="automation-transition-compatibility">
                  <header>
                    <h4>{copy('automation.transition.compatibility_fields')}</h4>
                    <span>{pendingTransition.preview.definition.compatibility_fields.length}</span>
                  </header>
                  <p>{copy('automation.transition.compatibility_fields_boundary')}</p>
                  <div>
                    <table>
                      <thead>
                        <tr>
                          <th scope="col">{copy('automation.transition.compatibility_address')}</th>
                          <th scope="col">{copy('automation.transition.compatibility_field')}</th>
                          <th scope="col">{copy('automation.transition.compatibility_disposition')}</th>
                          <th scope="col">{copy('automation.transition.compatibility_before')}</th>
                          <th scope="col">{copy('automation.transition.compatibility_after')}</th>
                          <th scope="col">{copy('automation.transition.compatibility_issue')}</th>
                        </tr>
                      </thead>
                      <tbody>
                        {pendingTransition.preview.definition.compatibility_fields.map((field) => (
                          <tr
                            key={`${field.address}:${field.field}`}
                            data-disposition={field.disposition}
                          >
                            <th scope="row"><code>{field.address}</code></th>
                            <td>{copy(`automation.transition.compatibility_field.${field.field}`)}</td>
                            <td>{copy(`automation.transition.compatibility_disposition.${field.disposition}`)}</td>
                            <td><code>{shortIdentity(field.before_digest)}</code></td>
                            <td><code>{shortIdentity(field.after_digest)}</code></td>
                            <td>{field.issue_code
                              ? copy(`automation.transition.compatibility.${field.issue_code}`)
                              : copy('automation.transition.compatibility_field_clear')}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </section>
              ) : null}
              {pendingTransition.preview.definition.compatibility_issues.length > 0 ? (
                <section className="automation-transition-issues">
                  <h4>{copy('automation.transition.compatibility_issues')}</h4>
                  <ol>
                    {pendingTransition.preview.definition.compatibility_issues.map((issue, index) => (
                      <li key={`${issue.code}:${issue.address ?? 'none'}:${index}`}>
                        {copy(`automation.transition.compatibility.${issue.code}`)}
                        {issue.address ? <code>{issue.address}</code> : null}
                      </li>
                    ))}
                  </ol>
                </section>
              ) : <p>{copy('automation.transition.compatibility_clear')}</p>}
              {!pendingTransition.preview.definition.commit_allowed ? (
                <p role="status">{copy('automation.transition.status.commit_blocked')}</p>
              ) : null}
            </>
          ) : null}
          <div>
            <button
              type="button"
              onClick={() => {
                setPendingTransition(null);
                onTransitionPreviewChange(null);
              }}
              disabled={saving}
            >{copy('automation.transition.cancel')}</button>
            <button
              type="button"
              onClick={() => { void applyTransition(); }}
              disabled={saving
                || !pendingTransition.preview
                || !pendingTransition.preview.definition.commit_allowed}
            >{copy(`automation.transition.confirm.${pendingTransition.operation}`)}</button>
          </div>
        </section>
      ) : null}
    </details>
  );
}

function firstObservedDifference(
  retained: readonly MechanismTimelineEntry[],
  current: readonly MechanismTimelineEntry[],
): { retained: MechanismTimelineEntry | null; current: MechanismTimelineEntry | null } | null {
  const length = Math.max(retained.length, current.length);
  for (let index = 0; index < length; index += 1) {
    const left = retained[index] ?? null;
    const right = current[index] ?? null;
    if (!left || !right || JSON.stringify(left.event) !== JSON.stringify(right.event)) {
      return { retained: left, current: right };
    }
  }
  return null;
}

function breakpointLabel(breakpoint: CommissionBreakpoint): string {
  switch (breakpoint.kind) {
    case 'event': return `${copy('automation.breakpoint.event')} ${copy(`automation.event.${breakpoint.eventKind}`)}`;
    case 'object': return `${copy('automation.breakpoint.object')} ${copy(`field.inspect.${breakpoint.objectKind}`)} ${breakpoint.objectId}`;
    case 'rule': return `${copy('automation.breakpoint.rule')} ${breakpoint.address}:${breakpoint.rule + 1}`;
    case 'outcome': return `${copy('automation.breakpoint.outcome')} ${copy(`automation.outcome.${breakpoint.outcome}`)}`;
    case 'criterion': return copy('automation.breakpoint.criterion');
  }
}

type ProgrammableInspection = Extract<FieldInspection, { target: 'form' | 'node' }>;
type PolicyCapabilities = ProgrammableInspection['policy_capabilities'];

interface DraftDiagnostic {
  key: string;
  rule: number | 'fallback';
  detail?: string;
}

function conditionDiagnostics(
  condition: LocalCondition,
  rule: number,
  capabilities: PolicyCapabilities,
  routes: readonly FrameRoute[],
): DraftDiagnostic[] {
  const diagnostics: DraftDiagnostic[] = [];
  if (!capabilities.conditions.includes(condition.kind)) {
    diagnostics.push({ key: 'automation.diagnostic.condition_unavailable', rule });
  }
  if (condition.kind === 'supply' || condition.kind === 'signal_present') {
    if (condition.radius > capabilities.sensor_radius_max) {
      diagnostics.push({
        key: 'automation.diagnostic.sensor_radius',
        rule,
        detail: `${fixed(capabilities.sensor_radius_max)} ${copy('unit.field_units')}`,
      });
    }
  }
  if (condition.kind === 'target_in_range'
      && condition.radius > capabilities.coupling_radius_max) {
    diagnostics.push({
      key: 'automation.diagnostic.coupling_radius',
      rule,
      detail: `${fixed(capabilities.coupling_radius_max)} ${copy('unit.field_units')}`,
    });
  }
  if (condition.kind === 'route_flow_below' || condition.kind === 'route_flow_above') {
    if (!routes.some((route) => route.route === condition.route)) {
      diagnostics.push({
        key: 'automation.diagnostic.route_not_attached',
        rule,
        detail: `${copy('automation.label.route')} ${condition.route}`,
      });
    }
  }
  return diagnostics;
}

function actionDiagnostics(
  action: LocalAction,
  rule: number | 'fallback',
  capabilities: PolicyCapabilities,
  routes: readonly FrameRoute[],
): DraftDiagnostic[] {
  const diagnostics: DraftDiagnostic[] = [];
  if (!capabilities.actions.includes(action.kind)) {
    diagnostics.push({ key: 'automation.diagnostic.action_unavailable', rule });
  }
  if ((action.kind === 'seek_supply' || action.kind === 'seek_port' || action.kind === 'seek_signal')
      && action.radius > capabilities.sensor_radius_max) {
    diagnostics.push({
      key: 'automation.diagnostic.sensor_radius',
      rule,
      detail: `${fixed(capabilities.sensor_radius_max)} ${copy('unit.field_units')}`,
    });
  }
  if (action.kind === 'couple' && action.radius > capabilities.coupling_radius_max) {
    diagnostics.push({
      key: 'automation.diagnostic.coupling_radius',
      rule,
      detail: `${fixed(capabilities.coupling_radius_max)} ${copy('unit.field_units')}`,
    });
  }
  if (action.kind === 'set_route') {
    const route = routes.find((held) => held.route === action.route);
    if (!route) {
      diagnostics.push({
        key: 'automation.diagnostic.route_not_owned',
        rule,
        detail: `${copy('automation.label.route')} ${action.route}`,
      });
    } else if (action.capacity_limit > route.capacity) {
      diagnostics.push({
        key: 'automation.diagnostic.route_capacity',
        rule,
        detail: `${fixed(route.capacity)} ${copy('unit.cu_per_step')}`,
      });
    }
    if (action.allocation_weight < 1 || action.allocation_weight > capabilities.route_weight_max) {
      diagnostics.push({ key: 'automation.diagnostic.route_weight', rule });
    }
  }
  if (action.kind === 'emit_signal'
      && (action.strength < 1 || action.strength > capabilities.signal_strength_max)) {
    diagnostics.push({ key: 'automation.diagnostic.signal_strength', rule });
  }
  return diagnostics;
}

function draftDiagnostics(
  draft: ComponentPolicy | null,
  capabilities: PolicyCapabilities | null,
  localRoutes: readonly FrameRoute[],
  ownedRoutes: readonly FrameRoute[],
): DraftDiagnostic[] {
  if (!draft || !capabilities) return [];
  const diagnostics = draft.rules.flatMap((rule, position) => [
    ...conditionDiagnostics(rule.condition, position, capabilities, localRoutes),
    ...actionDiagnostics(rule.action, position, capabilities, ownedRoutes),
  ]);
  diagnostics.push(...actionDiagnostics(draft.fallback, 'fallback', capabilities, ownedRoutes));
  return diagnostics;
}

function rejectionStatus(answer: Extract<ResponseEnvelope, { ok: false }>): string {
  if (answer.error.detail?.reason === 'stale_base') {
    return copy('automation.status.refused_stale');
  }
  const field = typeof answer.error.detail?.field === 'string'
    ? answer.error.detail.field
    : 'unknown';
  const keys: Record<string, string> = {
    action: 'automation.status.refused_action',
    address: 'automation.status.refused_address',
    allocation_weight: 'automation.status.refused_weight',
    capabilities: 'automation.status.refused_capabilities',
    capacity_limit: 'automation.status.refused_capacity',
    controller: 'automation.status.refused_controller',
    local_policy: 'automation.status.refused_policy',
    radius: 'automation.status.refused_radius',
    route: 'automation.status.refused_route',
    route_defaults: 'automation.status.refused_route_defaults',
  };
  return copy(keys[field] ?? 'automation.status.refused');
}

function restartRejectionStatus(answer: Extract<ResponseEnvelope, { ok: false }>): string {
  const field = typeof answer.error.detail?.field === 'string'
    ? answer.error.detail.field
    : 'unknown';
  const keys: Record<string, string> = {
    assembly_template: 'automation.restart.refused_assembly',
    assembly_template_hash: 'automation.restart.refused_stale',
    branch_id: 'automation.restart.refused_stale',
    branch_nonce: 'automation.restart.refused_stale',
    generator_spec_hash: 'automation.restart.refused_stale',
    run_kind: 'automation.restart.refused_run_kind',
  };
  return copy(keys[field] ?? 'automation.restart.refused');
}

export function AutomationWorkbench({
  contract,
  identity,
  mode,
  rate,
  step,
  queue,
  tool,
  policy,
  routeDefaults,
  selection,
  routes,
  criterion,
  pressures,
  mechanismEvents,
  commissionHistory,
  commissionArchiveError,
  qualificationRequestArchiveError,
  qualificationExecutionArchiveError,
  qualificationJob,
  qualificationTrialArtifacts,
  qualificationCriterionDecisions,
  qualificationFunctionDecision,
  qualificationGrades,
  qualificationFailureTrace,
  qualificationResult,
  qualificationReceipt,
  blueprints,
  generatorSources,
  generatorSourceReadError,
  engineeringMigration,
  engineeringRecoveries,
  commissionBreakpoint,
  commissionBreakpointHit,
  inspectionStep,
  selectedEventOrdinal,
  onDesign,
  onCommission,
  onPreviewRestart,
  onPreviewQualification,
  onFreezeQualification,
  onRetryQualificationPersistence,
  onRetryQualificationExecutionPersistence,
  onStartQualification,
  onCancelQualification,
  onResolveQualification,
  onGradeQualification,
  onTraceQualificationFailure,
  onAssembleQualificationResult,
  onProjectQualificationProgress,
  onCaptureBlueprint,
  onReadEngineeringAssembly,
  onPreviewEngineeringAssembly,
  onAssemblyPreviewChange,
  onCommitEngineeringAssembly,
  onPreviewEngineeringTransition,
  onTransitionPreviewChange,
  onCommitEngineeringTransition,
  onRestart,
  onRate,
  onTool,
  onUndo,
  onCommit,
  onDeployJunction,
  onOpenContracts,
  onOpenLab,
  onSetBreakpoint,
  onPreviewPolicy,
  onPreviewChange,
  onSelectEvent,
  onApplyPolicy,
}: AutomationWorkbenchProps) {
  const [expanded, setExpanded] = useState(true);
  const [draft, setDraft] = useState<ComponentPolicy | null>(null);
  const [baseline, setBaseline] = useState('');
  const [routeDraft, setRouteDraft] = useState<RouteControlDefault | null>(null);
  const [routeBaseline, setRouteBaseline] = useState('');
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [restartState, setRestartState] = useState<RestartUiState>('closed');
  const [restartPreview, setRestartPreview] = useState<CommissionRestartPreview | null>(null);
  const [restartStatus, setRestartStatus] = useState<string | null>(null);
  const [qualificationPreview, setQualificationPreview] = useState<QualificationInputPreview | null>(null);
  const [qualificationStatus, setQualificationStatus] = useState<'idle' | 'loading' | 'error'>('idle');
  const [qualificationRefresh, setQualificationRefresh] = useState(0);
  const [preview, setPreview] = useState<PolicyPreview | null>(null);
  const [previewStep, setPreviewStep] = useState<number | null>(null);
  const [dismissedGuidance, setDismissedGuidance] = useState<string[]>([]);
  const [criterionTrail, setCriterionTrail] = useState<{
    branchId: string | null;
    readings: CriterionReading[];
  }>({ branchId: null, readings: [] });
  const address = componentAddress(selection);
  const mobile = selection?.target === 'form';
  const localRoutes = useMemo(
    () => address === null ? [] : attachedRoutes(address, routes),
    [address, routes],
  );
  const ownedRoutes = useMemo(
    () => address === null ? [] : outgoingRoutes(address, routes),
    [address, routes],
  );
  const committedRouteDefaults = useMemo(() => {
    const defaults = routeDefaults.length > 0
      ? routeDefaults.map((control) => ({ ...control }))
      : embodiedRouteDefaults(routes);
    return defaults.sort((first, second) => first.route - second.route);
  }, [routeDefaults, routes]);
  const committedRouteDefaultsKey = JSON.stringify(committedRouteDefaults);
  const ownedRoutesKey = JSON.stringify(ownedRoutes.map((route) => [
    route.route,
    route.tail,
    route.head,
    route.capacity,
  ]));
  const selectedRoute = selection?.target === 'route' ? selection : null;
  const selectedRouteDefault = selectedRoute
    ? committedRouteDefaults.find((control) => control.route === selectedRoute.id) ?? null
    : null;
  const selectedRouteDefaultKey = selectedRouteDefault
    ? JSON.stringify(selectedRouteDefault)
    : selectedRoute
      ? JSON.stringify([
          selectedRoute.id,
          selectedRoute.enabled,
          selectedRoute.capacity_limit,
          selectedRoute.allocation_weight,
          selectedRoute.controller,
        ])
      : '';

  useEffect(() => {
    if (address === null) {
      setDraft(null);
      setBaseline('');
      return;
    }
    const held = policy.components.find((component) => component.address === address)
      ?? defaultPolicy(address, mobile, ownedRoutes, contract?.id ?? null);
    const serialized = JSON.stringify(held);
    setDraft(held);
    setBaseline(serialized);
    setStatus(null);
  }, [address, contract?.id, mobile, ownedRoutesKey, policy]);

  useEffect(() => {
    if (!selectedRoute) {
      setRouteDraft(null);
      setRouteBaseline('');
      return;
    }
    const held = selectedRouteDefault ?? {
      route: selectedRoute.id,
      enabled: selectedRoute.enabled,
      capacity_limit: selectedRoute.capacity_limit,
      allocation_weight: selectedRoute.allocation_weight,
      controller: selectedRoute.controller,
    };
    const serialized = JSON.stringify(held);
    setRouteDraft({ ...held });
    setRouteBaseline(serialized);
    setStatus(null);
  }, [selectedRoute?.id, selectedRouteDefaultKey]);

  const paused = mode === 'still';
  const qualificationFrozen = identity?.qualificationRequest !== null
    && identity?.qualificationRequest !== undefined;
  const designing = paused && inspectionStep === null && !qualificationFrozen;
  const dirty = draft !== null && JSON.stringify(draft) !== baseline;
  const routeDirty = routeDraft !== null && JSON.stringify(routeDraft) !== routeBaseline;
  const programmable = selection?.target === 'form' || selection?.target === 'node'
    ? selection
    : null;
  const hardwareCapabilities = programmable?.policy_capabilities ?? null;
  const capabilities = useMemo<PolicyCapabilities | null>(() => {
    if (!hardwareCapabilities) return null;
    if (!contract) return hardwareCapabilities;
    return {
      ...hardwareCapabilities,
      actions: hardwareCapabilities.actions.filter((kind) =>
        contract.capabilities.actions.includes(kind)),
      conditions: hardwareCapabilities.conditions.filter((kind) =>
        contract.capabilities.conditions.includes(kind)),
    };
  }, [contract, hardwareCapabilities]);
  const availableConditions = capabilities?.conditions ?? CONDITION_KINDS;
  const availableActions = capabilities?.actions ?? (mobile ? MOBILE_ACTIONS : STATIONARY_ACTIONS);
  const ruleMaximum = contract?.limits.max_rules_per_component ?? MAX_RULES;
  const routeActuationAvailable = contract?.capabilities.hardware.includes('route_actuator') ?? true;
  const selectionProgrammable = programmable === null || !contract
    ? programmable !== null
    : programmable.target === 'form'
      ? contract.capabilities.hardware.includes('mobile_component')
      : contract.capabilities.hardware.includes('interface_actuator')
        || contract.capabilities.hardware.includes('route_actuator');
  const couplingMaximum = capabilities?.coupling_radius_max ?? 192 * WHOLE;
  const sensorMaximum = capabilities?.sensor_radius_max ?? 4_096 * WHOLE;
  const signalMaximum = capabilities?.signal_strength_max ?? Number.MAX_SAFE_INTEGER;
  const diagnostics = useMemo(
    () => draftDiagnostics(draft, capabilities, localRoutes, ownedRoutes),
    [capabilities, draft, localRoutes, ownedRoutes],
  );
  const activeRule = programmable?.policy_rule ?? -1;
  const activeAction = programmable?.policy_action ?? null;
  const guidance = contextualGuidance(contract, policy, criterion, mechanismEvents, selection);
  const shownGuidance = guidance && !dismissedGuidance.includes(guidance.id) ? guidance : null;
  const supplyCycle = contract?.opening.supply_cycles.find((cycle) => cycle.duty < WHOLE) ?? null;
  const selectedMechanism = mechanismEvents.find((entry) => entry.ordinal === selectedEventOrdinal) ?? null;
  const breakpointObject = selection && 'id' in selection
    && (selection.target === 'form'
      || selection.target === 'node'
      || selection.target === 'route'
      || selection.target === 'current')
    ? { kind: 'object' as const, objectKind: selection.target, objectId: selection.id }
    : null;
  const priorCriterion = criterionTrail.readings.length > 1
    ? criterionTrail.readings[criterionTrail.readings.length - 2]
    : null;
  const restartStale = restartPreview !== null && (
    restartPreview.branch_id !== identity?.branchId
    || restartPreview.branch_nonce !== identity?.branchNonce
    || restartPreview.generator_spec_hash !== identity?.generatorHash
    || restartPreview.assembly_template_hash !== identity?.assemblyHash
  );
  const qualificationStale = qualificationPreview !== null && (
    qualificationPreview.input.branch_id !== identity?.branchId
    || qualificationPreview.input.branch_nonce !== identity?.branchNonce
    || qualificationPreview.input.generator_spec_hash !== identity?.generatorHash
    || qualificationPreview.input.assembly_template_hash !== identity?.assemblyHash
  );

  useEffect(() => setDismissedGuidance([]), [contract?.id]);

  useEffect(() => {
    if (!contract || !designing) {
      setQualificationPreview(null);
      setQualificationStatus('idle');
      return;
    }
    let current = true;
    setQualificationStatus('loading');
    void onPreviewQualification().then((answer) => {
      if (!current) return;
      if (!answer.ok) {
        setQualificationPreview(null);
        setQualificationStatus('error');
        return;
      }
      setQualificationPreview(answer.body as QualificationInputPreview);
      setQualificationStatus('idle');
    });
    return () => { current = false; };
  }, [
    contract?.id,
    designing,
    identity?.assemblyHash,
    identity?.branchId,
    identity?.branchNonce,
    identity?.generatorHash,
    onPreviewQualification,
    qualificationRefresh,
  ]);

  useEffect(() => {
    const branchId = identity?.branchId ?? null;
    setCriterionTrail((held) => {
      if (!criterion) return held.branchId === branchId && held.readings.length === 0
        ? held
        : { branchId, readings: [] };
      if (held.branchId !== branchId) return { branchId, readings: [criterion] };
      if (held.readings[held.readings.length - 1]?.step === criterion.step) return held;
      return { branchId, readings: [...held.readings.slice(-31), criterion] };
    });
  }, [criterion?.step, identity?.branchId]);

  useEffect(() => {
    if (!designing || !draft || address === null || !capabilities || diagnostics.length > 0) {
      setPreview(null);
      setPreviewStep(null);
      onPreviewChange(null);
      return;
    }
    let current = true;
    const timer = globalThis.setTimeout(() => {
      const next = policyWithComponent(policy, draft);
      void onPreviewPolicy(address, next, committedRouteDefaults).then((answer) => {
        if (!current) return;
        if (!answer.ok) {
          setPreview(null);
          setPreviewStep(null);
          onPreviewChange(null);
          return;
        }
        const body = answer.body as DesignPreviewed;
        setPreview(body.preview);
        setPreviewStep(body.snapshot_step);
        onPreviewChange(body.preview);
      });
    }, 90);
    return () => {
      current = false;
      globalThis.clearTimeout(timer);
    };
  }, [
    address,
    capabilities,
    designing,
    diagnostics.length,
    draft,
    committedRouteDefaultsKey,
    onPreviewChange,
    onPreviewPolicy,
    policy,
  ]);

  const updateRule = (position: number, next: PolicyRule): void => {
    setDraft((held) => held ? {
      ...held,
      rules: held.rules.map((rule, index) => index === position ? next : rule),
    } : held);
  };

  const moveRule = (position: number, by: -1 | 1): void => {
    setDraft((held) => {
      if (!held) return held;
      const target = position + by;
      if (target < 0 || target >= held.rules.length) return held;
      const rules = [...held.rules];
      [rules[position], rules[target]] = [rules[target], rules[position]];
      return { ...held, rules };
    });
  };

  const removeRule = (position: number): void => {
    setDraft((held) => held ? {
      ...held,
      rules: held.rules.filter((_, index) => index !== position),
    } : held);
  };

  const copyRule = (position: number): void => {
    setDraft((held) => {
      if (!held || held.rules.length >= (contract?.limits.max_rules_per_component ?? MAX_RULES)) {
        return held;
      }
      const rules = [...held.rules];
      rules.splice(position + 1, 0, {
        ...held.rules[position],
        condition: { ...held.rules[position].condition },
        action: { ...held.rules[position].action },
      });
      return { ...held, rules };
    });
  };

  const toggleRule = (position: number): void => {
    setDraft((held) => held ? {
      ...held,
      rules: held.rules.map((rule, index) => index === position
        ? { ...rule, enabled: !rule.enabled }
        : rule),
    } : held);
  };

  const apply = async (): Promise<void> => {
    if (!draft || !designing || diagnostics.length > 0) return;
    setSaving(true);
    setStatus(copy('automation.status.installing'));
    const next = policyWithComponent(policy, draft);
    const answer = await onApplyPolicy(next, committedRouteDefaults);
    setSaving(false);
    if (answer.ok) {
      setBaseline(JSON.stringify(draft));
      setStatus(copy('automation.status.installed'));
    } else {
      setStatus(rejectionStatus(answer));
    }
  };

  const disable = async (): Promise<void> => {
    if (address === null || !designing) return;
    setSaving(true);
    const next: FrozenLocalPolicy = {
      version: 2,
      components: policy.components.filter((component) => component.address !== address),
    };
    const answer = await onApplyPolicy(next, committedRouteDefaults);
    setSaving(false);
    setStatus(answer.ok ? copy('automation.status.disabled') : rejectionStatus(answer));
  };

  const applyRouteDefault = async (): Promise<void> => {
    if (!routeDraft || !designing || !routeDirty) return;
    setSaving(true);
    setStatus(copy('automation.status.installing'));
    const nextDefaults = committedRouteDefaults
      .map((control) => control.route === routeDraft.route ? { ...routeDraft } : control)
      .sort((first, second) => first.route - second.route);
    const answer = await onApplyPolicy(policy, nextDefaults);
    setSaving(false);
    if (answer.ok) {
      setRouteBaseline(JSON.stringify(routeDraft));
      setStatus(copy('automation.status.route_installed'));
    } else {
      setStatus(rejectionStatus(answer));
    }
  };

  const openRestart = async (): Promise<void> => {
    if (!designing || restartState !== 'closed') return;
    setRestartState('previewing');
    setRestartPreview(null);
    setRestartStatus(null);
    const answer = await onPreviewRestart();
    if (!answer.ok) {
      setRestartState('ready');
      setRestartStatus(restartRejectionStatus(answer));
      return;
    }
    setRestartPreview(answer.body as CommissionRestartPreview);
    setRestartState('ready');
  };

  const closeRestart = (): void => {
    if (restartState === 'submitting') return;
    setRestartState('closed');
    setRestartPreview(null);
    setRestartStatus(null);
  };

  const confirmRestart = async (): Promise<void> => {
    if (!restartPreview || restartState !== 'ready') return;
    if (restartStale) {
      setRestartStatus(copy('automation.restart.stale'));
      return;
    }
    setRestartState('submitting');
    setRestartStatus(copy('automation.restart.submitting'));
    const answer = await onRestart(restartPreview);
    if (!answer.ok) {
      setRestartState('ready');
      setRestartStatus(restartRejectionStatus(answer));
      return;
    }
    setRestartState('closed');
    setRestartPreview(null);
    setRestartStatus(null);
  };

  return (
    <>
      <header className="automation-command-bar" data-mode={mode ?? 'opening'}>
        <div className="automation-contract">
          <span>{copy('automation.contract.label')}</span>
          <strong>{contract ? copy(contract.title_key) : copy('automation.contract.open_field')}</strong>
          <small>{contract ? copy(contract.brief_key) : copy('automation.contract.open_field_criterion')}</small>
          <em>{copy('automation.criterion.current')} {currentConstraint(contract, criterion)}</em>
        </div>
        <div className="automation-authority" role="group" aria-label={copy('automation.authority.label')}>
          <button type="button" data-active={paused && !qualificationFrozen} onClick={onDesign} disabled={qualificationFrozen}>
            {copy(qualificationFrozen ? 'automation.authority.frozen' : 'automation.authority.design')}
          </button>
          <button type="button" data-active={mode === 'running'} onClick={onCommission} disabled={qualificationFrozen}>
            {copy('automation.authority.commission')}
          </button>
          <button type="button" onClick={() => { void openRestart(); }} disabled={!designing || restartState !== 'closed'}>
            {copy('automation.command.restart_commission')}
          </button>
        </div>
        <div className="automation-rate" role="group" aria-label={copy('automation.rate.label')}>
          {([1, 4, 16] as const).map((choice) => (
            <button
              type="button"
              key={choice}
              data-active={rate === choice}
              onClick={() => onRate(choice)}
              disabled={paused || qualificationFrozen}
            >
              {choice}{copy('unit.multiplier')}
            </button>
          ))}
        </div>
        <dl className="automation-run-reading">
          <div><dt>{copy('automation.label.step')}</dt><dd>{step.toLocaleString('en-US')}</dd></div>
          <div><dt>{copy('automation.label.authority')}</dt><dd>{copy(qualificationFrozen ? 'automation.authority.frozen' : inspectionStep !== null ? 'automation.authority.history' : paused ? 'automation.authority.design' : 'automation.authority.commission')}</dd></div>
        </dl>
        <button
          type="button"
          className="automation-panel-toggle"
          aria-expanded={expanded}
          onClick={() => setExpanded((held) => !held)}
        >
          {copy(qualificationFrozen
            ? expanded ? 'automation.command.hide_request' : 'automation.command.show_request'
            : expanded ? 'automation.command.hide_editor' : 'automation.command.show_editor')}
        </button>
      </header>

      {restartState !== 'closed' ? (
        <div className="automation-restart-backdrop">
          <section
            className="automation-restart-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="automation-restart-title"
            data-state={restartState}
          >
            <header>
              <div>
                <span>{copy('automation.restart.boundary')}</span>
                <h2 id="automation-restart-title">{copy('automation.restart.title')}</h2>
              </div>
              <strong>{copy(`automation.restart.state.${restartState}`)}</strong>
            </header>
            {restartState === 'previewing' ? (
              <p role="status">{copy('automation.restart.previewing')}</p>
            ) : null}
            {restartPreview ? (
              <>
                <p>{copy('automation.restart.explanation')}</p>
                <dl className="automation-restart-identities">
                  <div><dt>{copy('automation.identity.branch_id')}</dt><dd><code>{restartPreview.branch_id}</code></dd></div>
                  <div><dt>{copy('automation.restart.current_step')}</dt><dd>{restartPreview.current_step.toLocaleString('en-US')}</dd></div>
                  <div><dt>{copy('contract.ladder.generator')}</dt><dd><code>{restartPreview.generator_spec_hash}</code></dd></div>
                  <div><dt>{copy('contract.ladder.assembly')}</dt><dd><code>{restartPreview.assembly_template_hash}</code></dd></div>
                  <div><dt>{copy('automation.restart.current_embodied')}</dt><dd><code>{restartPreview.current_embodied_state_hash}</code></dd></div>
                  <div><dt>{copy('automation.restart.next_nonce')}</dt><dd>{restartPreview.predicted_branch_nonce.toLocaleString('en-US')}</dd></div>
                </dl>
                <ol className="automation-restart-consequences">
                  {(['keep_generator', 'restore_assembly', 'retain_evidence', 'create_child_branch'] as const).map((consequence) => (
                    <li key={consequence}>
                      <span>{copy(`automation.restart.${consequence}.label`)}</span>
                      <strong>{copy(`automation.restart.${consequence}.detail`)}</strong>
                    </li>
                  ))}
                </ol>
                {restartStale ? <p role="alert">{copy('automation.restart.stale')}</p> : null}
              </>
            ) : null}
            {restartStatus ? <p role="status">{restartStatus}</p> : null}
            <footer>
              <button type="button" onClick={closeRestart} disabled={restartState === 'submitting'}>
                {copy('automation.restart.cancel')}
              </button>
              <button
                type="button"
                className="automation-restart-confirm"
                onClick={() => { void confirmRestart(); }}
                disabled={!restartPreview || restartState !== 'ready' || restartStale}
              >
                {copy('automation.restart.confirm')}
              </button>
            </footer>
          </section>
        </div>
      ) : null}

      {expanded ? (
        <aside
          className="automation-editor"
          data-frozen={qualificationFrozen}
          aria-label={copy(qualificationFrozen ? 'automation.qualification.frozen_title' : 'automation.editor.label')}
        >
          <header className="automation-editor-heading">
            <span>{copy(qualificationFrozen ? 'automation.qualification.seal' : 'automation.editor.label')}</span>
            <strong>{qualificationFrozen
              ? copy('automation.authority.frozen')
              : selection ? selectionName(selection) : copy('automation.selection.none')}</strong>
          </header>
          {contract?.id === 'intake' ? (
            <section className="automation-process-chain" aria-label={copy('automation.intake.chain_label')}>
              <span>{copy('automation.intake.chain_sense')}</span>
              <span>{copy('automation.intake.chain_acquire')}</span>
              <span>{copy('automation.intake.chain_couple')}</span>
              <span>{copy('automation.intake.chain_service')}</span>
            </section>
          ) : null}
          {contract?.id === 'transfer' ? (
            <section className="automation-process-chain" aria-label={copy('automation.transfer.chain_label')}>
              <span>{copy('automation.transfer.chain_source')}</span>
              <span>{copy('automation.transfer.chain_stage')}</span>
              <span>{copy('automation.transfer.chain_gate')}</span>
              <span>{copy('automation.transfer.chain_accept')}</span>
            </section>
          ) : null}
          {contract?.id === 'buffer' ? (
            <section className="automation-process-chain" data-steps="5" aria-label={copy('automation.buffer.chain_label')}>
              <span>{copy('automation.buffer.chain_phase')} {supplyCycle
                ? `${supplyCycle.current}: ${supplyCycle.on_steps} ${copy('contract.ladder.emitting')} / ${supplyCycle.period - supplyCycle.on_steps} ${copy('contract.ladder.quiet')}`
                : copy('automation.buffer.chain_phase_unavailable')}</span>
              <span>{copy('automation.buffer.chain_bank')}</span>
              <span>{copy('automation.buffer.chain_release')}</span>
              <span>{copy('automation.buffer.chain_bridge')}</span>
              <span>{copy('automation.buffer.chain_service')}</span>
            </section>
          ) : null}
          {shownGuidance ? (
            <aside className="automation-guidance" aria-live="polite">
              <div>
                <span>{copy('automation.guidance.label')}</span>
                {shownGuidance.address !== null
                  ? <b>{copy('automation.label.address')} {shownGuidance.address}</b>
                  : null}
              </div>
              <p>{copy(shownGuidance.key)}</p>
              <button
                type="button"
                onClick={() => setDismissedGuidance((held) => [...held, shownGuidance.id])}
              >
                {copy('automation.guidance.dismiss')}
              </button>
            </aside>
          ) : null}
          {!selection ? (
            <p className="automation-empty">{copy('automation.selection.prompt')}</p>
          ) : address === null ? (
            <>
              <ObjectReading selection={selection} />
              {selection.target === 'route' && routeDraft && routeActuationAvailable ? (
                <section className="automation-route-default">
                  <header>
                    <span>{copy('automation.route.default_label')}</span>
                    <small>{copy('automation.route.default_explanation')}</small>
                  </header>
                  <fieldset disabled={!designing}>
                    <label className="automation-toggle">
                      <input
                        type="checkbox"
                        checked={routeDraft.enabled}
                        onChange={(event) => setRouteDraft({
                          ...routeDraft,
                          enabled: event.target.checked,
                        })}
                      />
                      <span>{copy(routeDraft.enabled
                        ? 'automation.route.enabled'
                        : 'automation.route.disabled')}</span>
                    </label>
                    <UnitInput
                      label="automation.label.route_limit"
                      value={routeDraft.capacity_limit}
                      maximum={selection.capacity}
                      onChange={(capacity_limit) => setRouteDraft({
                        ...routeDraft,
                        capacity_limit,
                      })}
                    />
                    <label className="automation-parameter">
                      <span>{copy('automation.label.allocation_weight')}</span>
                      <input
                        type="number"
                        min="1"
                        max="65535"
                        step="1"
                        value={routeDraft.allocation_weight}
                        onChange={(event) => setRouteDraft({
                          ...routeDraft,
                          allocation_weight: integer(event.target.value, 1),
                        })}
                      />
                    </label>
                  </fieldset>
                  <footer className="automation-editor-actions">
                    <button
                      type="button"
                      className="automation-apply"
                      onClick={() => void applyRouteDefault()}
                      disabled={!designing || saving || !routeDirty}
                    >
                      {copy('automation.command.apply_route_default')}
                    </button>
                    {status ? <p role="status">{status}</p> : null}
                  </footer>
                </section>
              ) : selection.target === 'route' && !routeActuationAvailable ? (
                <p className="automation-empty">{copy('automation.route.read_only_contract')}</p>
              ) : null}
            </>
          ) : !selectionProgrammable ? (
            <>
              <ObjectReading selection={selection} />
              <p className="automation-empty">{copy('automation.selection.contract_read_only')}</p>
            </>
          ) : draft ? (
            <>
              <ObjectReading selection={selection} />
              {capabilities ? (
                <section className="automation-capability">
                  <header>
                    <span>{copy('automation.capability.label')}</span>
                    <strong>{copy(capabilities.mobile
                      ? 'automation.capability.mobile'
                      : 'automation.capability.stationary')}</strong>
                  </header>
                  <dl>
                    <div>
                      <dt>{copy('automation.capability.sensor')}</dt>
                      <dd>{fixed(capabilities.sensor_radius_max)} {copy('unit.field_units')}</dd>
                    </div>
                    <div>
                      <dt>{copy('automation.capability.coupling')}</dt>
                      <dd>{capabilities.mobile
                        ? `${fixed(capabilities.coupling_radius_max)} ${copy('unit.field_units')}`
                        : copy('field.inspect.unavailable')}</dd>
                    </div>
                    <div>
                      <dt>{copy('automation.capability.routes')}</dt>
                      <dd>{capabilities.attached_routes.filter((route) => route.outgoing).length}</dd>
                    </div>
                    <div>
                      <dt>{copy('automation.capability.actions')}</dt>
                      <dd>{capabilities.actions.length}</dd>
                    </div>
                  </dl>
                </section>
              ) : null}
              <section className="automation-active">
                <header><span>{copy('automation.active.label')}</span><b>{activeRule >= 0 ? activeRule + 1 : copy('automation.active.fallback')}</b></header>
                <strong>{actionLabel(activeAction)}</strong>
                {programmable ? <small>{copy('automation.outcome.label')} {copy(`automation.outcome.${programmable.policy_outcome}`)}</small> : null}
                {programmable && programmable.policy_target_kind !== 'none' ? (
                  <small>{copy('automation.active.target')} {programmable.policy_target_kind} {programmable.policy_target}</small>
                ) : null}
                {programmable ? <small>{copy('automation.label.runtime_timer')} {programmable.policy_timer} {copy('unit.steps')} · {copy('automation.label.cooldown')} {programmable.policy_cooldown} {copy('unit.steps')}</small> : null}
              </section>
              {preview ? (
                <section className="automation-preview">
                  <header>
                    <span>{copy('automation.preview.label')}</span>
                    <b>{preview.rule >= 0
                      ? `${copy('automation.rule.label')} ${preview.rule + 1}`
                      : copy('automation.active.fallback')}</b>
                  </header>
                  <strong>{actionLabel(preview.action)}</strong>
                  <small>
                    {copy('automation.preview.snapshot')} {previewStep ?? '-'} ·{' '}
                    {copy('automation.preview.candidates')} {preview.candidates.length}
                  </small>
                  <small>{preview.target_kind === 'none'
                    ? copy('automation.preview.no_target')
                    : `${copy('automation.preview.target')} ${preview.target_kind} ${preview.target ?? '-'}`}</small>
                </section>
              ) : null}
              <section className="automation-rules" aria-label={copy('automation.rules.label')}>
                <header>
                  <div>
                    <span>{copy('automation.rules.label')}</span>
                    <strong>{draft.rules.length} / {ruleMaximum}</strong>
                  </div>
                  <button
                    type="button"
                    disabled={!designing || draft.rules.length >= ruleMaximum}
                    onClick={() => setDraft({
                      ...draft,
                      rules: [...draft.rules, {
                        enabled: true,
                        condition: conditionFrom('always', localRoutes),
                        action: actionFrom('hold', ownedRoutes),
                      }],
                    })}
                  >
                    {copy('automation.command.add_rule')}
                  </button>
                </header>
                <ol>
                  {draft.rules.map((rule, position) => (
                    <li
                      key={position}
                      data-active={activeRule === position}
                      data-enabled={rule.enabled}
                    >
                      <header>
                        <span>
                          {copy('automation.rule.label')} {position + 1}
                          <small>{copy(rule.enabled ? 'automation.rule.enabled' : 'automation.rule.disabled')}</small>
                        </span>
                        <div>
                          <button type="button" disabled={!designing || position === 0} onClick={() => moveRule(position, -1)}>{copy('automation.command.earlier')}</button>
                          <button type="button" disabled={!designing || position === draft.rules.length - 1} onClick={() => moveRule(position, 1)}>{copy('automation.command.later')}</button>
                          <button
                            type="button"
                            disabled={!designing || draft.rules.length >= ruleMaximum}
                            onClick={() => copyRule(position)}
                            title={copy('automation.command.copy_rule')}
                          >
                            {copy('automation.command.copy_rule')}
                          </button>
                          <button
                            type="button"
                            disabled={!designing}
                            onClick={() => toggleRule(position)}
                            title={copy(rule.enabled ? 'automation.command.disable_rule' : 'automation.command.enable_rule')}
                          >
                            {copy(rule.enabled ? 'automation.command.disable_rule' : 'automation.command.enable_rule')}
                          </button>
                          <button type="button" disabled={!designing} onClick={() => removeRule(position)}>{copy('automation.command.remove')}</button>
                        </div>
                      </header>
                      <fieldset disabled={!designing}>
                        <ConditionEditor
                          condition={rule.condition}
                          routes={localRoutes}
                          available={availableConditions}
                          couplingMaximum={couplingMaximum}
                          sensorMaximum={sensorMaximum}
                          onChange={(condition) => updateRule(position, { ...rule, condition })}
                        />
                        <ActionEditor
                          action={rule.action}
                          mobile={mobile}
                          routes={ownedRoutes}
                          available={availableActions}
                          couplingMaximum={couplingMaximum}
                          sensorMaximum={sensorMaximum}
                          signalMaximum={signalMaximum}
                          onChange={(action) => updateRule(position, { ...rule, action })}
                        />
                      </fieldset>
                    </li>
                  ))}
                </ol>
              </section>
              <section className="automation-fallback">
                <header><span>{copy('automation.fallback.label')}</span><small>{copy('automation.fallback.explanation')}</small></header>
                <fieldset disabled={!designing}>
                  <ActionEditor
                    action={draft.fallback}
                    mobile={mobile}
                    routes={ownedRoutes}
                    available={availableActions}
                    couplingMaximum={couplingMaximum}
                    sensorMaximum={sensorMaximum}
                    signalMaximum={signalMaximum}
                    onChange={(fallback) => setDraft({ ...draft, fallback })}
                  />
                </fieldset>
              </section>
              {diagnostics.length > 0 ? (
                <section className="automation-diagnostics" aria-label={copy('automation.diagnostic.label')}>
                  <header>
                    <span>{copy('automation.diagnostic.label')}</span>
                    <strong>{diagnostics.length}</strong>
                  </header>
                  <ul>
                    {diagnostics.map((diagnostic, position) => (
                      <li key={`${diagnostic.rule}-${diagnostic.key}-${position}`}>
                        <b>{diagnostic.rule === 'fallback'
                          ? copy('automation.fallback.label')
                          : `${copy('automation.rule.label')} ${diagnostic.rule + 1}`}</b>
                        <span>{copy(diagnostic.key)}{diagnostic.detail ? ` ${diagnostic.detail}` : ''}</span>
                      </li>
                    ))}
                  </ul>
                </section>
              ) : null}
              <footer className="automation-editor-actions">
                <button type="button" onClick={() => void disable()} disabled={!designing || saving || !policy.components.some((component) => component.address === address)}>
                  {copy('automation.command.disable')}
                </button>
                <button type="button" className="automation-apply" onClick={() => void apply()} disabled={!designing || saving || !dirty || diagnostics.length > 0}>
                  {copy('automation.command.apply')}
                </button>
                {status ? <p role="status">{status}</p> : null}
              </footer>
            </>
          ) : null}
          {contract ? (
            <QualificationPreview
              preview={qualificationPreview}
              frozen={identity?.qualificationRequest ?? null}
              loading={qualificationStatus === 'loading'}
              error={qualificationStatus === 'error'}
              stale={qualificationStale}
              persistenceError={qualificationRequestArchiveError}
              executionPersistenceError={qualificationExecutionArchiveError}
              job={qualificationJob}
              trialArtifacts={qualificationTrialArtifacts}
              criterionDecisions={qualificationCriterionDecisions}
              functionDecision={qualificationFunctionDecision}
              grades={qualificationGrades}
              failureTrace={qualificationFailureTrace}
              result={qualificationResult}
              receipt={qualificationReceipt}
              onRefresh={() => setQualificationRefresh((held) => held + 1)}
              onFreeze={onFreezeQualification}
              onRetryPersistence={onRetryQualificationPersistence}
              onRetryExecutionPersistence={onRetryQualificationExecutionPersistence}
              onStartQualification={onStartQualification}
              onCancelQualification={onCancelQualification}
              onResolveQualification={onResolveQualification}
              onGradeQualification={onGradeQualification}
              onTraceQualificationFailure={onTraceQualificationFailure}
              onAssembleQualificationResult={onAssembleQualificationResult}
              onProjectQualificationProgress={onProjectQualificationProgress}
            />
          ) : null}
          <CriterionMarginRail reading={criterion} prior={priorCriterion} />
          {contract ? (
            <AttemptHistory
              records={commissionHistory}
              durabilityError={commissionArchiveError}
              currentIdentity={identity}
              currentCriterion={criterion}
              currentEvents={mechanismEvents}
            />
          ) : null}
          {contract ? (
            <EngineeringMemory
              records={blueprints}
              generatorSources={generatorSources}
              generatorSourceReadError={generatorSourceReadError}
              migration={engineeringMigration}
              recoveries={engineeringRecoveries}
              identity={identity}
              enabled={designing && !qualificationFrozen}
              onCapture={onCaptureBlueprint}
              onReadAssembly={onReadEngineeringAssembly}
              onPreviewAssembly={onPreviewEngineeringAssembly}
              onAssemblyPreviewChange={onAssemblyPreviewChange}
              onCommitAssembly={onCommitEngineeringAssembly}
              onPreviewTransition={onPreviewEngineeringTransition}
              onTransitionPreviewChange={onTransitionPreviewChange}
              onCommitTransition={onCommitEngineeringTransition}
            />
          ) : null}
        </aside>
      ) : null}

      <section className="automation-timeline" aria-label={copy('automation.timeline.label')}>
        <div className="automation-construction" data-frozen={qualificationFrozen}>
          {qualificationFrozen ? (
            <span className="automation-frozen-strip">{copy('automation.qualification.controls_withdrawn')}</span>
          ) : null}
          <div className="automation-tool" role="group" aria-label={copy('automation.tool.label')}>
            <button type="button" data-active={tool === 'view'} onClick={() => onTool('view')} disabled={!designing}>{copy('automation.tool.view')}</button>
            <button type="button" data-active={tool === 'compartment'} onClick={() => onTool('compartment')} disabled={!designing}>{copy('automation.tool.compartment')}</button>
          </div>
          <span>{queue.entries.length} {copy('automation.queue.edits')} · {queue.cost_total} {copy('label.impulse')}</span>
          <button type="button" onClick={onUndo} disabled={!designing || queue.entries.length === 0}>{copy('automation.command.undo')}</button>
          <button type="button" onClick={() => { void onCommit(); }} disabled={!designing || queue.entries.length === 0}>{copy('automation.command.commit')}</button>
          <button type="button" onClick={onDeployJunction} disabled={!designing}>{copy('automation.command.deploy')}</button>
          <button
            type="button"
            onClick={() => { void onOpenContracts(); }}
            disabled={qualificationFrozen && (
              !qualificationResult
              || (qualificationResult.result.definition.outcome === 'passed' && !qualificationReceipt)
            )}
          >
            {copy('contract.ladder.command')}
          </button>
          <button type="button" onClick={onOpenLab} disabled={qualificationFrozen}>{copy('automation.command.lab')}</button>
        </div>
        <div className="automation-evidence">
          <span data-state={criterion?.status ?? 'idle'}>
            {copy('automation.criterion.label')} {criterion ? copy(`automation.criterion.${criterion.status}`) : copy('automation.criterion.unassigned')}
          </span>
          <span>{copy('automation.criterion.components')} {criterion ? `${criterion.components.filter((component) => component.met).length}/${criterion.components.length}` : '-'}</span>
          <span>{copy('automation.criterion.routes')} {criterion ? `${criterion.routes.filter((route) => route.met).length}/${criterion.routes.length}` : '-'}</span>
          <span>{copy('automation.criterion.leakage')} {criterion?.leakage.ratio == null ? '-' : `${(criterion.leakage.ratio * 100 / WHOLE).toFixed(1)}%`}</span>
          <span>{copy('automation.pressure.label')} {pressures.length > 0 ? pressures.map((pressure) => copy(`pressure.${pressure.pressure}`)).join(' / ') : copy('automation.pressure.none')}</span>
        </div>
        <div className="automation-event-strip">
          <header>
            <span>{copy('automation.event.label')}</span>
            <b>{mechanismEvents.length}</b>
          </header>
          <div className="automation-breakpoints" aria-label={copy('automation.breakpoint.title')}>
            <span>{commissionBreakpoint
              ? breakpointLabel(commissionBreakpoint)
              : commissionBreakpointHit
                ? `${copy('automation.breakpoint.hit')} ${commissionBreakpointHit.step}`
                : copy('automation.breakpoint.none')}</span>
            <button
              type="button"
              disabled={!selectedMechanism}
              onClick={() => selectedMechanism && onSetBreakpoint({
                kind: 'event',
                eventKind: selectedMechanism.event.kind,
              })}
            >{copy('automation.breakpoint.arm_event')}</button>
            <button
              type="button"
              disabled={!breakpointObject}
              onClick={() => breakpointObject && onSetBreakpoint(breakpointObject)}
            >{copy('automation.breakpoint.arm_object')}</button>
            <button
              type="button"
              disabled={selectedMechanism?.event.kind !== 'policy'}
              onClick={() => {
                if (selectedMechanism?.event.kind !== 'policy') return;
                onSetBreakpoint({
                  kind: 'rule',
                  address: selectedMechanism.event.address,
                  rule: selectedMechanism.event.rule,
                });
              }}
            >{copy('automation.breakpoint.arm_rule')}</button>
            <button
              type="button"
              disabled={selectedMechanism?.event.kind !== 'policy'}
              onClick={() => {
                if (selectedMechanism?.event.kind !== 'policy') return;
                onSetBreakpoint({ kind: 'outcome', outcome: selectedMechanism.event.outcome });
              }}
            >{copy('automation.breakpoint.arm_outcome')}</button>
            <button type="button" onClick={() => onSetBreakpoint({ kind: 'criterion' })}>
              {copy('automation.breakpoint.arm_criterion')}
            </button>
            <button type="button" disabled={!commissionBreakpoint} onClick={() => onSetBreakpoint(null)}>
              {copy('automation.breakpoint.clear')}
            </button>
          </div>
          {mechanismEvents.length > 0 ? (
            <ol>
              {[...mechanismEvents].slice(-16).reverse().map((entry) => (
                <li key={entry.ordinal}>
                  <button
                    type="button"
                    data-active={selectedEventOrdinal === entry.ordinal}
                    disabled={!mechanismSelectable(entry)}
                    onClick={() => onSelectEvent(entry)}
                    title={mechanismLabel(entry)}
                  >
                    <span>{copy('automation.label.step')} {entry.step}</span>
                    <strong>{mechanismLabel(entry)}</strong>
                  </button>
                </li>
              ))}
            </ol>
          ) : <p>{copy('automation.event.empty')}</p>}
        </div>
      </section>
    </>
  );
}

function ObjectReading({ selection }: { selection: FieldInspection }) {
  if (selection.target === 'form' || selection.target === 'node') {
    return (
      <dl className="automation-object-reading">
        <div><dt>{copy('automation.label.address')}</dt><dd>{selection.target === 'form' ? selection.node : selection.id}</dd></div>
        <div><dt>{copy('field.inspect.layer')}</dt><dd>{selection.layer}</dd></div>
        <div><dt>{copy('field.inspect.charge')}</dt><dd>{fixed(selection.q)} / {fixed(selection.capacity)} {copy('unit.cu')}</dd></div>
        <div><dt>{copy('automation.label.interface')}</dt><dd>{selection.target === 'node' ? copy(selection.open ? 'automation.interface.open' : 'automation.interface.closed') : copy(selection.automated ? 'automation.status.automated' : 'automation.status.unprogrammed')}</dd></div>
        {selection.target === 'form' && selection.ability === 'reserve_discharge' ? (
          <div><dt>{copy('automation.reserve.level')}</dt><dd>{fixed(selection.ability_value)} / {fixed(selection.ability_limit)} {copy('unit.cu')}</dd></div>
        ) : null}
      </dl>
    );
  }
  if (selection.target === 'route') {
    return (
      <dl className="automation-object-reading">
        <div><dt>{copy('field.inspect.direction')}</dt><dd>{selection.tail} {copy('automation.label.to')} {selection.head}</dd></div>
        <div><dt>{copy('automation.route.state')}</dt><dd>{copy(selection.enabled ? 'automation.route.enabled' : 'automation.route.disabled')}</dd></div>
        <div><dt>{copy('automation.route.controller')}</dt><dd>{selection.controller}</dd></div>
        <div><dt>{copy('field.inspect.flow')}</dt><dd>{fixed(selection.flow)} {copy('unit.cu_per_step')}</dd></div>
        <div><dt>{copy('automation.label.requested_flow')}</dt><dd>{fixed(selection.requested_flow)} {copy('unit.cu_per_step')}</dd></div>
        <div><dt>{copy('automation.label.accepted_flow')}</dt><dd>{fixed(selection.accepted_flow)} {copy('unit.cu_per_step')}</dd></div>
        <div><dt>{copy('automation.route.outcome')}</dt><dd>{copy(`automation.route.${selection.outcome}`)}</dd></div>
        <div><dt>{copy('field.inspect.capacity')}</dt><dd>{fixed(selection.capacity)} {copy('unit.cu_per_step')}</dd></div>
        <div><dt>{copy('automation.label.route_limit')}</dt><dd>{fixed(selection.capacity_limit)} {copy('unit.cu_per_step')}</dd></div>
        <div><dt>{copy('automation.label.allocation_weight')}</dt><dd>{selection.allocation_weight}</dd></div>
      </dl>
    );
  }
  return <p className="automation-empty">{copy('automation.selection.read_only')}</p>;
}
