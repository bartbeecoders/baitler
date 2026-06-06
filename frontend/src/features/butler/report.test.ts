import { describe, expect, it } from 'vitest';

import type { ReportArtifact } from './api';
import { artifactPath, linkableArtifacts, relativeTime, reportBadges } from './report';

function artifact(over: Partial<ReportArtifact>): ReportArtifact {
  return {
    id: 'a1',
    agent: 'claude-code',
    run_id: 'r1',
    action: 'idea.create',
    target_type: 'idea',
    target_id: 'i1',
    target_title: 'An idea',
    project_id: null,
    summary: 'idea.create · An idea',
    created_at: '2026-06-05T10:00:00Z',
    ...over,
  };
}

describe('reportBadges', () => {
  it('folds per-action counts into typed badges, dropping zeros', () => {
    const badges = reportBadges({
      'idea.create': 4,
      'file.import': 10,
      'file.create': 2,
      'document.update': 1,
      'page.publish': 1,
      'knowledge.link': 3,
    });
    const byKey = Object.fromEntries(badges.map((b) => [b.key, b.count]));
    expect(byKey).toEqual({ ideas: 4, files: 12, links: 3, updated: 2 });
  });

  it('is empty for a read-only run', () => {
    expect(reportBadges({})).toEqual([]);
  });
});

describe('artifactPath', () => {
  it('deep-links each content type to its detail route', () => {
    expect(artifactPath(artifact({ target_type: 'idea', target_id: 'x' }))).toBe('/ideas/x');
    expect(artifactPath(artifact({ target_type: 'document', target_id: 'x' }))).toBe('/editor/x');
    expect(artifactPath(artifact({ target_type: 'page', target_id: 'x' }))).toBe('/pages/x');
    expect(artifactPath(artifact({ target_type: 'mindmap', target_id: 'x' }))).toBe('/mindmaps/x');
    expect(artifactPath(artifact({ target_type: 'file', target_id: 'x' }))).toBe('/files');
    expect(artifactPath(artifact({ target_type: 'project', target_id: 'x' }))).toBe('/projects');
    expect(artifactPath(artifact({ target_id: '' }))).toBeNull();
    expect(artifactPath(artifact({ target_type: 'link', target_id: 'x' }))).toBeNull();
  });
});

describe('linkableArtifacts', () => {
  it('dedupes by target and skips deletes/unlinks/untitled rows', () => {
    const rows = [
      artifact({ id: '1', target_id: 'i1' }),
      artifact({ id: '2', target_id: 'i1', action: 'idea.update' }), // dup target
      artifact({ id: '3', target_id: 'i2', action: 'idea.delete' }), // deleted
      artifact({ id: '4', target_id: 'i3', target_title: '' }), // untitled
      artifact({ id: '5', target_type: 'document', target_id: 'd1', action: 'document.create' }),
    ];
    const out = linkableArtifacts(rows);
    expect(out.map((a) => `${a.target_type}:${a.target_id}`)).toEqual(['idea:i1', 'document:d1']);
  });
});

describe('relativeTime', () => {
  const now = new Date('2026-06-05T12:00:00Z');
  it('renders compact relative stamps', () => {
    expect(relativeTime('2026-06-05T11:59:40Z', now)).toBe('just now');
    expect(relativeTime('2026-06-05T11:45:00Z', now)).toBe('15m ago');
    expect(relativeTime('2026-06-05T09:00:00Z', now)).toBe('3h ago');
    expect(relativeTime('2026-06-01T12:00:00Z', now)).toBe('4d ago');
    expect(relativeTime('not a date', now)).toBe('');
  });
});
