import authored from '../../../content/regimes/catalog.json';

export type RegimeId =
  | 'open_field'
  | 'periodic_transport'
  | 'crowded_medium'
  | 'vestige_pressure'
  | 'holdout_atmosphere';

export interface RegimeEntry {
  id: RegimeId;
  status: 'implemented' | 'pending';
  position: { x: number; y: number };
  lawset: string;
  supply_per_step: number;
  supply_duty: number;
  supply_jitter: number;
  supply_width: number;
  dissipation_per_step: number;
  conductance_noise: number;
  medium_velocity: { x: number; y: number };
  medium_drag: number;
  collision_radius: number;
  collision_response: number;
  mixing_steps: number;
  network: { nodes: number; routes: number };
  criterion: string;
  interventions: string[];
}

export const REGIMES = authored.regimes as RegimeEntry[];

export function regimeById(id: RegimeId): RegimeEntry {
  const regime = REGIMES.find((entry) => entry.id === id);
  if (!regime) throw new Error(`Unknown regime ${id}`);
  return regime;
}
