import { apiUrl } from './api';

export type LiveUpdateMessage =
  | { kind: 'match_updated'; match_id: number }
  | { kind: 'reporting_rounds_updated' };

interface SubscribeToLiveUpdatesOptions {
  matchId?: number;
  onMatchUpdated?: (matchId: number) => void;
  onReportingRoundsUpdated?: () => void;
}

export function subscribeToLiveUpdates(options: SubscribeToLiveUpdatesOptions) {
  const params = new URLSearchParams();
  if (typeof options.matchId === 'number') {
    params.set('match_id', String(options.matchId));
  }

  const query = params.toString();
  const source = new EventSource(apiUrl(`/v1/stream/live-updates${query ? `?${query}` : ''}`));

  source.addEventListener('update', (event) => {
    try {
      const update = JSON.parse((event as MessageEvent<string>).data) as LiveUpdateMessage;
      if (update.kind === 'match_updated') {
        options.onMatchUpdated?.(update.match_id);
        return;
      }

      if (update.kind === 'reporting_rounds_updated') {
        options.onReportingRoundsUpdated?.();
      }
    } catch {
    }
  });

  return () => {
    source.close();
  };
}