export type RenderQuality = 'low' | 'medium' | 'high';

export interface RenderQualityProfile {
  id: RenderQuality;
  dprCap: number;
  pixelCap: number;
  movingParticles: number;
  particlesPerSegment: number;
  persistentMarks: number;
  softMarks: boolean;
  backdropEtching: boolean;
  currentWash: boolean;
  routeGlow: boolean;
  formFacets: 0 | 1 | 2;
  pressureBloom: boolean;
}

const PROFILES: Readonly<Record<RenderQuality, RenderQualityProfile>> = {
  low: {
    id: 'low',
    dprCap: 1,
    pixelCap: 1_300_000,
    movingParticles: 80,
    particlesPerSegment: 2,
    persistentMarks: 32,
    softMarks: false,
    backdropEtching: false,
    currentWash: false,
    routeGlow: false,
    formFacets: 0,
    pressureBloom: false,
  },
  medium: {
    id: 'medium',
    dprCap: 1.5,
    pixelCap: 2_000_000,
    movingParticles: 180,
    particlesPerSegment: 4,
    persistentMarks: 64,
    softMarks: true,
    backdropEtching: true,
    currentWash: true,
    routeGlow: true,
    formFacets: 1,
    pressureBloom: true,
  },
  high: {
    id: 'high',
    dprCap: 2,
    pixelCap: 2_600_000,
    movingParticles: 320,
    particlesPerSegment: 7,
    persistentMarks: 96,
    softMarks: true,
    backdropEtching: true,
    currentWash: true,
    routeGlow: true,
    formFacets: 2,
    pressureBloom: true,
  },
};

let activeQuality: RenderQuality | null = null;

function preferredQuality(): RenderQuality {
  if (activeQuality) return activeQuality;
  if (typeof localStorage !== 'undefined') {
    try {
      const held = localStorage.getItem('field-render-quality');
      if (held === 'low' || held === 'medium' || held === 'high') {
        activeQuality = held;
        return held;
      }
    } catch {
      // Storage may be unavailable in a private embedded context.
    }
  }
  if (typeof navigator !== 'undefined') {
    const memory = (navigator as Navigator & { deviceMemory?: number }).deviceMemory;
    if (memory !== undefined && memory <= 4) activeQuality = 'low';
    else if (navigator.hardwareConcurrency <= 4) activeQuality = 'medium';
  }
  activeQuality ??= 'high';
  return activeQuality;
}

export function renderQuality(): RenderQualityProfile {
  return PROFILES[preferredQuality()];
}

export function boundedDensity(width: number, height: number, deviceDensity: number): number {
  const profile = renderQuality();
  const requested = Math.min(profile.dprCap, Math.max(1, deviceDensity));
  const area = Math.max(1, width * height);
  return Math.max(1, Math.min(requested, Math.sqrt(profile.pixelCap / area)));
}

export function setRenderQuality(quality: RenderQuality): void {
  activeQuality = quality;
  try {
    localStorage.setItem('field-render-quality', quality);
  } catch {
    // The active selection still applies for this Atlas session.
  }
}
