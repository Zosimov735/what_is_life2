import { copy } from './copy';
import type { ChapterChanged, ObjectiveState } from '../../../worker/src/protocol';

interface CampaignRailProps {
  chapter: ChapterChanged | null;
  objective: ObjectiveState | null;
  objectiveOrdinal: number;
  step: number;
}

function clock(step: number): string {
  const total = Math.max(0, Math.floor(step / 30));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
    : `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
}

export function CampaignRail({ chapter, objective, objectiveOrdinal, step }: CampaignRailProps) {
  if (!chapter || chapter.chapter_count <= 0) return null;
  const objectiveCount = Math.max(1, chapter.objective_count);
  const ordinal = Math.min(objectiveCount, Math.max(1, objectiveOrdinal));
  const objectiveShare = objective ? Math.max(0, Math.min(1, objective.progress / 65_536)) : 0;
  const chapterShare = Math.max(0, Math.min(1, (ordinal - 1 + objectiveShare) / objectiveCount));
  const campaignShare = Math.max(
    0,
    Math.min(1, (chapter.chapter_index + chapterShare) / chapter.chapter_count),
  );

  return (
    <aside className="campaign-rail" aria-label={copy('campaign.position')}>
      <header>
        <span>{copy('campaign.position')}</span>
        <strong>{copy(chapter.title_key)}</strong>
        <time>{clock(step)}</time>
      </header>
      <div className="campaign-meta">
        <span>
          {copy('campaign.chapter')} <b>{chapter.chapter_index + 1}</b> / {chapter.chapter_count}
        </span>
        <span>
          {copy('campaign.objective')} <b>{ordinal}</b> / {objectiveCount}
        </span>
      </div>
      <div className="campaign-progress" aria-hidden="true">
        <span style={{ transform: `scaleX(${campaignShare})` }} />
        <i style={{ transform: `scaleX(${chapterShare})` }} />
      </div>
    </aside>
  );
}
