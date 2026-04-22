'use client';

import { useEffect, useState, Suspense } from 'react';
import { useRouter, useSearchParams } from 'next/navigation';
import { ArrowLeftRight, ChevronLeft, Circle, RotateCcw, Save, Shield, AlertTriangle } from 'lucide-react';
import useSWR from 'swr';
import { Text } from '../components/Text';
import { useAuth } from '../auth-provider';
import { apiUrl } from '../lib/api';
import { getTeamAbbreviation } from '../lib/team-name';

interface UpcomingMatch {
  id: number;
  t1_id: number;
  t2_id: number;
  t1_name: string;
  t2_name: string;
  t1_abbreviation?: string | null;
  t2_abbreviation?: string | null;
  field_name: string;
  time: string;
  possession: number | null;
}

interface MatchEvent {
  id: number;
  player_id: number | null;
  player_name: string;
  team_id: number;
  event_type: number;
  actor_user_id: number | null;
  created_at: string;
}

interface MatchPlayer {
  id: number;
  name: string;
  team_id: number;
}

interface MatchDetail {
  id: number;
  t1_id: number;
  t2_id: number;
  t1_name: string;
  t2_name: string;
  t1_abbreviation?: string | null;
  t2_abbreviation?: string | null;
  t1_score: number;
  t2_score: number;
  possession: number | null;
  field_name: string;
  time: string;
  players: MatchPlayer[];
  events: MatchEvent[];
}

type MatchStatus = 'upcoming' | 'live' | 'ended';
type PanelKey = 't1' | 'log' | 't2';
type ConfirmAction = 'end' | 'save' | 'undo' | 'possession';

const EVENT_LABELS = ['Score', 'Assist', 'Block', 'Turnover'];
const EVENT_COLORS = ['text-green-500', 'text-sky-400', 'text-violet-400', 'text-amber-400'];

function getMatchStatus(match: UpcomingMatch | MatchDetail): MatchStatus {
  if (match.possession === null) return 'upcoming';
  if (match.possession >= 3) return 'ended';
  return 'live';
}

function formatTime(time: string) {
  if (!time) return '';
  const date = new Date(time);
  return `${date.toLocaleDateString('en-US', { weekday: 'short', day: 'numeric', month: 'short' })} - ${date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false })}`;
}

function formatLogTime(time: string) {
  return new Date(time).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false });
}

function ConfirmDialog({ title, message, onConfirm, onCancel }: { title: string; message: string; onConfirm: () => void; onCancel: () => void }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm px-4">
      <div className="w-full max-w-sm rounded-2xl border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-900 p-6 shadow-xl">
        <div className="flex items-center gap-3 mb-3">
          <AlertTriangle className="h-5 w-5 text-amber-500 shrink-0" />
          <h3 className="text-base font-semibold text-gray-900 dark:text-white">{title}</h3>
        </div>
        <p className="text-sm text-gray-600 dark:text-slate-400 mb-5">{message}</p>
        <div className="flex gap-3 justify-end">
          <button onClick={onCancel} className="rounded-lg px-4 py-2 text-sm font-medium text-gray-700 dark:text-slate-300 bg-gray-100 dark:bg-slate-800 hover:bg-gray-200 dark:hover:bg-slate-700 transition">
            Cancel
          </button>
          <button onClick={onConfirm} className="rounded-lg px-4 py-2 text-sm font-semibold text-white bg-amber-500 hover:bg-amber-400 transition">
            Confirm
          </button>
        </div>
      </div>
    </div>
  );
}

function AdminContent() {
  const { isLoggedIn, isAdmin, isPoc, token, isLoading } = useAuth();
  const router = useRouter();
  const searchParams = useSearchParams();
  const canReport = isAdmin || isPoc;
  const requestedMatchId = searchParams.get('match_id');

  const [activeMatchId, setActiveMatchId] = useState<number | null>(null);
  const [panel, setPanel] = useState<PanelKey>('log');
  const [pendingScorerId, setPendingScorerId] = useState<number | null>(null);
  const [pendingAssisterId, setPendingAssisterId] = useState<number | null>(null);
  const [pendingBlockId, setPendingBlockId] = useState<number | null>(null);
  const [pendingTurnover, setPendingTurnover] = useState(false);
  const [pendingTurnoverId, setPendingTurnoverId] = useState<number | null>(null);
  const [pendingSwitchOnly, setPendingSwitchOnly] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [choosingPossession, setChoosingPossession] = useState<number | null>(null);
  const [confirmAction, setConfirmAction] = useState<ConfirmAction | null>(null);
  const [pendingPossession, setPendingPossession] = useState<{ matchId: number; possession: 1 | 2 } | null>(null);

  useEffect(() => {
    if (isLoading) return;
    if (!isLoggedIn) { router.push('/login'); return; }
    if (!canReport) router.push('/');
  }, [canReport, isLoading, isLoggedIn, router]);

  useEffect(() => {
    if (!requestedMatchId) return;
    const parsed = Number.parseInt(requestedMatchId, 10);
    if (!Number.isNaN(parsed)) setActiveMatchId(parsed);
  }, [requestedMatchId]);

  const fetchVolunteerMatches = async (): Promise<UpcomingMatch[]> => {
    if (!token) return [];
    const response = await fetch(apiUrl('/v1/admin/matches'), { headers: { Authorization: `Bearer ${token}` } });
    if (!response.ok) return [];
    return response.json();
  };

  const fetchMatchDetail = async (url: string): Promise<MatchDetail | null> => {
    const response = await fetch(url);
    if (!response.ok) return null;
    return response.json();
  };

  const { data: matches = [], mutate: mutateMatches, isLoading: matchesLoading } = useSWR(
    token && isLoggedIn && canReport ? apiUrl('/v1/admin/matches') : null,
    fetchVolunteerMatches,
    { refreshInterval: 4000, revalidateOnFocus: true }
  );

  const { data: activeMatch, mutate: mutateActiveMatch } = useSWR(
    activeMatchId ? apiUrl(`/v1/matches/${activeMatchId}`) : null,
    fetchMatchDetail,
    { refreshInterval: 3000, revalidateOnFocus: true }
  );

  useEffect(() => {
    if (!activeMatch || getMatchStatus(activeMatch) !== 'live') return;
    setPanel(activeMatch.possession === 1 ? 't1' : 't2');
    resetComposer();
  }, [activeMatch?.possession]);

  async function postAction(path: string, body?: Record<string, number | null>) {
    if (!token) throw new Error('Missing auth token');
    const response = await fetch(path, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${token}`,
        ...(body ? { 'Content-Type': 'application/json' } : {}),
      },
      ...(body ? { body: JSON.stringify(body) } : {}),
    });
    if (!response.ok) throw new Error('Request failed');
  }

  function resetComposer() {
    setPendingScorerId(null);
    setPendingAssisterId(null);
    setPendingBlockId(null);
    setPendingTurnover(false);
    setPendingTurnoverId(null);
    setPendingSwitchOnly(false);
    setErrorMessage(null);
  }

  async function startReporting(matchId: number, possession: 1 | 2) {
    try {
      setIsSubmitting(true);
      const response = await fetch(apiUrl(`/v1/admin/matches/${matchId}/start`), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify({ possession }),
      });
      if (!response.ok && response.status !== 409) throw new Error('Unable to start match');
      setActiveMatchId(matchId);
      setChoosingPossession(null);
      setPendingPossession(null);
      await mutateMatches();
      await mutateActiveMatch();
    } catch {
      setErrorMessage('Unable to start reporting right now.');
    } finally {
      setIsSubmitting(false);
    }
  }

  function requestStartReporting(matchId: number, possession: 1 | 2) {
    setPendingPossession({ matchId, possession });
    setConfirmAction('possession');
  }

  async function undoLastEvent() {
    if (!activeMatchId) return;
    try {
      setIsSubmitting(true);
      await postAction(apiUrl(`/v1/admin/matches/${activeMatchId}/undo`));
      resetComposer();
      await mutateActiveMatch();
      await mutateMatches();
    } catch {
      setErrorMessage('Unable to undo the last event.');
    } finally {
      setIsSubmitting(false);
    }
  }

  async function endMatch() {
    if (!activeMatchId) return;
    try {
      setIsSubmitting(true);
      await postAction(apiUrl(`/v1/admin/matches/${activeMatchId}/end`));
      resetComposer();
      setActiveMatchId(null);
      await mutateMatches();
    } catch {
      setErrorMessage('Unable to end the match right now.');
    } finally {
      setIsSubmitting(false);
    }
  }

  async function saveAction() {
    if (!activeMatchId || !activeMatch || panel === 'log') return;
    const isOffView =
      (panel === 't1' && activeMatch.possession === 1) ||
      (panel === 't2' && activeMatch.possession === 2);
    try {
      setIsSubmitting(true);
      if (pendingSwitchOnly) {
        await postAction(apiUrl(`/v1/admin/matches/${activeMatchId}/switch-possession`));
      } else if (isOffView && pendingTurnover) {
        await postAction(apiUrl(`/v1/admin/matches/${activeMatchId}/event`), { player_id: null, event_type: 3 });
      } else if (isOffView && pendingTurnoverId !== null) {
        await postAction(apiUrl(`/v1/admin/matches/${activeMatchId}/event`), { player_id: pendingTurnoverId, event_type: 3 });
      } else if (isOffView && pendingScorerId !== null && pendingAssisterId !== null && pendingScorerId !== pendingAssisterId) {
        await postAction(apiUrl(`/v1/admin/matches/${activeMatchId}/event`), { player_id: pendingScorerId, event_type: 0 });
        await postAction(apiUrl(`/v1/admin/matches/${activeMatchId}/event`), { player_id: pendingAssisterId, event_type: 1 });
      } else if (!isOffView && pendingBlockId !== null) {
        await postAction(apiUrl(`/v1/admin/matches/${activeMatchId}/event`), { player_id: pendingBlockId, event_type: 2 });
      }
      resetComposer();
      await mutateActiveMatch();
      await mutateMatches();
    } catch {
      setErrorMessage('Unable to save that action.');
    } finally {
      setIsSubmitting(false);
    }
  }

  function handlePlayerTap(playerId: number, action: 'scorer' | 'assister' | 'block' | 'turnover') {
    if (action === 'scorer') {
      setPendingScorerId(prev => prev === playerId ? null : playerId);
      setPendingTurnover(false);
      setPendingTurnoverId(null);
      setPendingSwitchOnly(false);
    } else if (action === 'assister') {
      setPendingAssisterId(prev => prev === playerId ? null : playerId);
      setPendingTurnover(false);
      setPendingTurnoverId(null);
      setPendingSwitchOnly(false);
    } else if (action === 'block') {
      setPendingBlockId(prev => prev === playerId ? null : playerId);
      setPendingSwitchOnly(false);
    } else if (action === 'turnover') {
      setPendingTurnoverId(prev => prev === playerId ? null : playerId);
      setPendingScorerId(null);
      setPendingAssisterId(null);
      setPendingTurnover(false);
      setPendingSwitchOnly(false);
    }
  }

  function activateTurnover() {
    setPendingScorerId(null);
    setPendingAssisterId(null);
    setPendingBlockId(null);
    setPendingTurnoverId(null);
    setPendingSwitchOnly(false);
    setPendingTurnover(prev => !prev);
  }

  function toggleSwitchOnly() {
    setPendingScorerId(null);
    setPendingAssisterId(null);
    setPendingBlockId(null);
    setPendingTurnover(false);
    setPendingTurnoverId(null);
    setPendingSwitchOnly(prev => !prev);
  }

  if (isLoading || matchesLoading) {
    return (
      <div className="min-h-screen px-4 py-4 md:px-0">
        <div className="mx-auto max-w-5xl"><Text variant="primary">Loading...</Text></div>
      </div>
    );
  }

  const matchStatus = activeMatch ? getMatchStatus(activeMatch) : null;
  const viewedTeamId = panel === 't1' ? activeMatch?.t1_id : activeMatch?.t2_id;
  const viewedPlayers = (activeMatch?.players.filter(p => p.team_id === viewedTeamId) ?? []).sort((a, b) => a.name.localeCompare(b.name));
  const isOffenseView = !!activeMatch && panel !== 'log' && (
    (panel === 't1' && activeMatch.possession === 1) ||
    (panel === 't2' && activeMatch.possession === 2)
  );
  const saveEnabled = !!activeMatch && panel !== 'log' && !isSubmitting && (
    pendingSwitchOnly ||
    (isOffenseView && pendingTurnover) ||
    (isOffenseView && pendingTurnoverId !== null) ||
    (isOffenseView && pendingScorerId !== null && pendingAssisterId !== null && pendingScorerId !== pendingAssisterId) ||
    (!isOffenseView && pendingBlockId !== null)
  );

  function getSaveLabel() {
    if (pendingSwitchOnly) return 'Switch';
    if (pendingTurnover || pendingTurnoverId !== null) return 'Save Turnover';
    if (isOffenseView) return 'Save Score';
    return 'Save Block';
  }

  function handleConfirm() {
    if (confirmAction === 'end') { endMatch(); }
    else if (confirmAction === 'save') { saveAction(); }
    else if (confirmAction === 'undo') { undoLastEvent(); }
    else if (confirmAction === 'possession' && pendingPossession) { startReporting(pendingPossession.matchId, pendingPossession.possession); }
    setConfirmAction(null);
  }

  // LIVE REPORTING VIEW
  if (activeMatch && matchStatus === 'live') {
    const t1Abbr = getTeamAbbreviation(activeMatch.t1_name, activeMatch.t1_abbreviation, 12);
    const t2Abbr = getTeamAbbreviation(activeMatch.t2_name, activeMatch.t2_abbreviation, 12);

    return (
      <div className="min-h-screen px-3 py-3 md:px-0">
        <div className="mx-auto max-w-2xl space-y-3">
          {/* Confirm dialog */}
          {confirmAction && (
            <ConfirmDialog
              title={confirmAction === 'end' ? 'End Match' : confirmAction === 'undo' ? 'Undo Event' : 'Save Event'}
              message={
                confirmAction === 'end' ? 'Are you sure you want to end this match? This cannot be easily reversed.'
                  : confirmAction === 'undo' ? 'Undo the last recorded event?'
                  : 'Save this event to the match log?'
              }
              onConfirm={handleConfirm}
              onCancel={() => setConfirmAction(null)}
            />
          )}

          {/* Top bar */}
          <div className="flex items-center justify-between">
            <button
              onClick={() => setActiveMatchId(null)}
              className="inline-flex items-center gap-1.5 rounded-full border border-gray-300 dark:border-slate-700 bg-white dark:bg-slate-900 px-3 py-1.5 text-sm text-gray-700 dark:text-slate-200 transition hover:bg-gray-50 dark:hover:bg-slate-800"
            >
              <ChevronLeft className="h-4 w-4" /> Back
            </button>
            <button
              onClick={() => setConfirmAction('end')}
              disabled={isSubmitting}
              className="rounded-full bg-red-600 px-5 py-2 text-sm font-semibold text-white transition hover:bg-red-500 disabled:opacity-60"
            >
              End Match
            </button>
          </div>

          {/* Score header */}
          <div className="rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-4">
            <div className="grid grid-cols-[1fr_auto_1fr] items-center gap-2">
              <button
                onClick={() => { setPanel('t1'); resetComposer(); }}
                className={`rounded-xl border p-3 text-left transition ${
                  panel === 't1' ? 'border-amber-400 bg-amber-50 dark:bg-amber-400/10' : 'border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800'
                }`}
              >
                <div className="text-sm font-semibold text-gray-900 dark:text-white truncate">{activeMatch.t1_name}</div>
                <div className={`text-[10px] font-bold tracking-widest mt-0.5 ${activeMatch.possession === 1 ? 'text-amber-600 dark:text-amber-400' : 'text-sky-600 dark:text-sky-400'}`}>
                  {activeMatch.possession === 1 ? 'OFFENSE' : 'DEFENSE'}
                </div>
                <div className="text-4xl font-black text-gray-900 dark:text-white mt-2">{activeMatch.t1_score}</div>
              </button>
              <div className="text-lg text-gray-400 dark:text-slate-500 font-light">-</div>
              <button
                onClick={() => { setPanel('t2'); resetComposer(); }}
                className={`rounded-xl border p-3 text-right transition ${
                  panel === 't2' ? 'border-amber-400 bg-amber-50 dark:bg-amber-400/10' : 'border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800'
                }`}
              >
                <div className="text-sm font-semibold text-gray-900 dark:text-white truncate">{activeMatch.t2_name}</div>
                <div className={`text-[10px] font-bold tracking-widest mt-0.5 ${activeMatch.possession === 2 ? 'text-amber-600 dark:text-amber-400' : 'text-sky-600 dark:text-sky-400'}`}>
                  {activeMatch.possession === 2 ? 'OFFENSE' : 'DEFENSE'}
                </div>
                <div className="text-4xl font-black text-gray-900 dark:text-white mt-2">{activeMatch.t2_score}</div>
              </button>
            </div>
          </div>

          {/* Tab slider */}
          <div className="grid grid-cols-3 rounded-full border border-gray-200 dark:border-slate-800 bg-gray-100 dark:bg-slate-900 p-1">
            {([
              { key: 't1' as PanelKey, label: t1Abbr },
              { key: 'log' as PanelKey, label: 'Log' },
              { key: 't2' as PanelKey, label: t2Abbr },
            ]).map(item => (
              <button
                key={item.key}
                onClick={() => { setPanel(item.key); resetComposer(); }}
                className={`truncate rounded-full px-2 py-2 text-sm font-semibold transition ${
                  panel === item.key
                    ? 'bg-amber-400 text-gray-900 dark:text-slate-950'
                    : 'text-gray-600 dark:text-slate-400 hover:bg-gray-200 dark:hover:bg-slate-800'
                }`}
              >
                {item.label}
              </button>
            ))}
          </div>

          {errorMessage && (
            <div className="rounded-xl border border-red-300 dark:border-red-500/40 bg-red-50 dark:bg-red-500/10 px-4 py-2">
              <Text className="text-sm text-red-700 dark:text-red-200">{errorMessage}</Text>
            </div>
          )}

          {/* Panel content - no inner scroll, full expansion */}
          {panel === 'log' ? (
            <div className="space-y-1.5">
              {activeMatch.events.length === 0 && (
                <Text className="text-sm text-gray-400 dark:text-slate-500 px-1">No events yet</Text>
              )}
              {activeMatch.events.slice().reverse().map(event => {
                const isT1 = event.team_id === activeMatch.t1_id;
                const playerName = event.player_name || (isT1 ? activeMatch.t1_name : activeMatch.t2_name);
                return (
                  <div key={event.id} className={`flex ${isT1 ? 'justify-start' : 'justify-end'}`}>
                    <div className={`inline-flex items-center gap-2 rounded-lg px-3 py-2 ${
                      isT1 ? 'bg-gray-100 dark:bg-slate-800/80' : 'bg-sky-50 dark:bg-sky-500/10'
                    }`}>
                      <span className={`text-xs font-bold ${EVENT_COLORS[event.event_type]}`}>{EVENT_LABELS[event.event_type]}</span>
                      <span className="text-sm text-gray-900 dark:text-white">{playerName}</span>
                      <span className="text-[10px] text-gray-400 dark:text-slate-500">{formatLogTime(event.created_at)}</span>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : isOffenseView ? (
            <div className="rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
              {/* Action bar */}
              <div className="flex items-center justify-between border-b border-gray-200 dark:border-slate-700 px-4 py-3">
                <button
                  onClick={() => setConfirmAction('undo')}
                  disabled={isSubmitting || activeMatch.events.length === 0}
                  className="inline-flex items-center gap-2 rounded-lg bg-gray-100 dark:bg-slate-800 px-4 py-2.5 text-sm font-semibold text-gray-700 dark:text-slate-200 hover:bg-gray-200 dark:hover:bg-slate-700 disabled:opacity-40 transition"
                >
                  <RotateCcw className="h-4 w-4" /> Undo
                </button>
                <button
                  onClick={() => setConfirmAction('save')}
                  disabled={!saveEnabled}
                  className={`inline-flex items-center gap-2 rounded-lg px-5 py-2.5 text-sm font-bold transition ${
                    saveEnabled ? 'bg-amber-400 text-gray-900 hover:bg-amber-300' : 'bg-gray-200 dark:bg-slate-800 text-gray-400 dark:text-slate-500'
                  }`}
                >
                  <Save className="h-4 w-4" /> {getSaveLabel()}
                </button>
              </div>
              {/* Table header */}
              <div className="grid grid-cols-[1fr_48px_48px_48px] border-b border-gray-200 dark:border-slate-700 px-4 py-2 text-[10px] font-bold tracking-wider text-gray-500 dark:text-slate-500 uppercase">
                <div>Player</div>
                <div className="text-center">Score</div>
                <div className="text-center">Assist</div>
                <div className="text-center">Turn</div>
              </div>
              {/* Player rows */}
              {viewedPlayers.map(player => {
                const isScorer = pendingScorerId === player.id;
                const isAssister = pendingAssisterId === player.id;
                const isTurnover = pendingTurnoverId === player.id;
                return (
                  <div
                    key={player.id}
                    className={`grid grid-cols-[1fr_48px_48px_48px] items-center border-b border-gray-100 dark:border-slate-800 px-4 py-2.5 ${
                      isScorer || isAssister || isTurnover ? 'bg-amber-50 dark:bg-amber-400/5' : ''
                    }`}
                  >
                    <div className="text-sm font-medium text-gray-900 dark:text-white truncate pr-2">{player.name}</div>
                    <button
                      onClick={() => handlePlayerTap(player.id, 'scorer')}
                      className={`mx-auto h-8 w-8 rounded-full text-xs font-bold transition ${
                        isScorer ? 'bg-green-500 text-white ring-2 ring-green-300' : 'bg-gray-100 dark:bg-slate-800 text-gray-400 dark:text-slate-500 hover:bg-green-100 dark:hover:bg-green-900/30'
                      }`}
                    >
                      {isScorer ? 'S' : ''}
                    </button>
                    <button
                      onClick={() => handlePlayerTap(player.id, 'assister')}
                      className={`mx-auto h-8 w-8 rounded-full text-xs font-bold transition ${
                        isAssister ? 'bg-sky-500 text-white ring-2 ring-sky-300' : 'bg-gray-100 dark:bg-slate-800 text-gray-400 dark:text-slate-500 hover:bg-sky-100 dark:hover:bg-sky-900/30'
                      }`}
                    >
                      {isAssister ? 'A' : ''}
                    </button>
                    <button
                      onClick={() => handlePlayerTap(player.id, 'turnover')}
                      className={`mx-auto h-8 w-8 rounded-full text-xs font-bold transition ${
                        isTurnover ? 'bg-amber-500 text-white ring-2 ring-amber-300' : 'bg-gray-100 dark:bg-slate-800 text-gray-400 dark:text-slate-500 hover:bg-amber-100 dark:hover:bg-amber-900/30'
                      }`}
                    >
                      {isTurnover ? 'T' : ''}
                    </button>
                  </div>
                );
              })}
              {/* No-player turnover */}
              <button
                onClick={activateTurnover}
                className={`w-full grid grid-cols-[1fr_48px_48px_48px] items-center px-4 py-2.5 transition text-left border-b border-gray-100 dark:border-slate-800 ${
                  pendingTurnover ? 'bg-amber-100 dark:bg-amber-400/10' : 'hover:bg-gray-50 dark:hover:bg-slate-800'
                }`}
              >
                <span className={`text-sm font-medium ${pendingTurnover ? 'text-amber-700 dark:text-amber-300' : 'text-gray-500 dark:text-slate-400'}`}>Turnover (no player)</span>
                <span /><span />
                <span className={`mx-auto h-8 w-8 rounded-full flex items-center justify-center text-xs font-bold ${
                  pendingTurnover ? 'bg-amber-400 text-gray-900 ring-2 ring-amber-300' : 'bg-gray-100 dark:bg-slate-800 text-gray-400 dark:text-slate-500'
                }`}>T</span>
              </button>
              {/* Switch possession */}
              <button
                onClick={toggleSwitchOnly}
                className={`w-full flex items-center justify-between px-4 py-3 transition ${
                  pendingSwitchOnly ? 'bg-sky-50 dark:bg-sky-400/10' : 'hover:bg-gray-50 dark:hover:bg-slate-800'
                }`}
              >
                <span className={`text-sm font-medium ${pendingSwitchOnly ? 'text-sky-700 dark:text-sky-300' : 'text-gray-500 dark:text-slate-400'}`}>Don&apos;t Know</span>
                <ArrowLeftRight className={`h-4 w-4 ${pendingSwitchOnly ? 'text-sky-600 dark:text-sky-300' : 'text-gray-400 dark:text-slate-500'}`} />
              </button>
              {pendingScorerId !== null && pendingAssisterId !== null && pendingScorerId === pendingAssisterId && (
                <div className="px-4 py-2 text-sm text-red-600 dark:text-red-300">Scorer and assister must be different</div>
              )}
            </div>
          ) : (
            <div className="rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
              {/* Action bar */}
              <div className="flex items-center justify-between border-b border-gray-200 dark:border-slate-700 px-4 py-3">
                <button
                  onClick={() => setConfirmAction('undo')}
                  disabled={isSubmitting || activeMatch.events.length === 0}
                  className="inline-flex items-center gap-2 rounded-lg bg-gray-100 dark:bg-slate-800 px-4 py-2.5 text-sm font-semibold text-gray-700 dark:text-slate-200 hover:bg-gray-200 dark:hover:bg-slate-700 disabled:opacity-40 transition"
                >
                  <RotateCcw className="h-4 w-4" /> Undo
                </button>
                <button
                  onClick={() => setConfirmAction('save')}
                  disabled={!saveEnabled}
                  className={`inline-flex items-center gap-2 rounded-lg px-5 py-2.5 text-sm font-bold transition ${
                    saveEnabled ? 'bg-amber-400 text-gray-900 hover:bg-amber-300' : 'bg-gray-200 dark:bg-slate-800 text-gray-400 dark:text-slate-500'
                  }`}
                >
                  <Save className="h-4 w-4" /> {pendingSwitchOnly ? 'Switch' : 'Save Block'}
                </button>
              </div>
              {/* Table header */}
              <div className="grid grid-cols-[1fr_56px] border-b border-gray-200 dark:border-slate-700 px-4 py-2 text-[10px] font-bold tracking-wider text-gray-500 dark:text-slate-500 uppercase">
                <div>Player</div>
                <div className="text-center">Block</div>
              </div>
              {/* Player rows */}
              {viewedPlayers.map(player => {
                const isBlock = pendingBlockId === player.id;
                return (
                  <div
                    key={player.id}
                    className={`grid grid-cols-[1fr_56px] items-center border-b border-gray-100 dark:border-slate-800 px-4 py-2.5 ${
                      isBlock ? 'bg-violet-50 dark:bg-violet-400/5' : ''
                    }`}
                  >
                    <div className="text-sm font-medium text-gray-900 dark:text-white truncate pr-2">{player.name}</div>
                    <button
                      onClick={() => handlePlayerTap(player.id, 'block')}
                      className={`mx-auto h-8 w-8 rounded-full text-xs font-bold transition ${
                        isBlock ? 'bg-violet-500 text-white ring-2 ring-violet-300' : 'bg-gray-100 dark:bg-slate-800 text-gray-400 dark:text-slate-500 hover:bg-violet-100 dark:hover:bg-violet-900/30'
                      }`}
                    >
                      {isBlock ? 'B' : ''}
                    </button>
                  </div>
                );
              })}
              {/* Switch possession */}
              <button
                onClick={toggleSwitchOnly}
                className={`w-full flex items-center justify-between px-4 py-3 transition ${
                  pendingSwitchOnly ? 'bg-sky-50 dark:bg-sky-400/10' : 'hover:bg-gray-50 dark:hover:bg-slate-800'
                }`}
              >
                <span className={`text-sm font-medium ${pendingSwitchOnly ? 'text-sky-700 dark:text-sky-300' : 'text-gray-500 dark:text-slate-400'}`}>Don&apos;t Know</span>
                <ArrowLeftRight className={`h-4 w-4 ${pendingSwitchOnly ? 'text-sky-600 dark:text-sky-300' : 'text-gray-400 dark:text-slate-500'}`} />
              </button>
            </div>
          )}
        </div>
      </div>
    );
  }

  // POSSESSION CHOOSER
  if (choosingPossession !== null) {
    const match = matches.find(m => m.id === choosingPossession);
    if (match) {
      return (
        <div className="min-h-screen px-4 py-4 md:px-0">
          <div className="mx-auto max-w-lg">
            {/* Confirm dialog for possession */}
            {confirmAction === 'possession' && pendingPossession && (
              <ConfirmDialog
                title="Start Match"
                message={`Set ${pendingPossession.possession === 1 ? match.t1_name : match.t2_name} as starting on offense? This can be undone later.`}
                onConfirm={handleConfirm}
                onCancel={() => { setConfirmAction(null); setPendingPossession(null); }}
              />
            )}
            <div className="rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6">
              <button
                onClick={() => setChoosingPossession(null)}
                className="mb-4 inline-flex items-center gap-1.5 text-sm text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white"
              >
                <ChevronLeft className="h-4 w-4" /> Back
              </button>
              <div className="text-lg font-semibold text-gray-900 dark:text-white">{match.t1_name} vs {match.t2_name}</div>
              <div className="text-sm text-gray-500 dark:text-slate-400 mt-1">{formatTime(match.time)} &bull; {match.field_name || 'Field pending'}</div>

              <div className="mt-6 text-sm font-medium text-gray-700 dark:text-slate-300">Who starts on offense?</div>
              <div className="mt-3 grid grid-cols-2 gap-3">
                <button
                  onClick={() => requestStartReporting(match.id, 1)}
                  disabled={isSubmitting}
                  className="rounded-xl border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800 px-4 py-4 text-sm font-semibold text-gray-900 dark:text-white transition hover:border-amber-400 hover:bg-amber-50 dark:hover:bg-amber-400/10 disabled:opacity-60"
                >
                  {match.t1_name}
                </button>
                <button
                  onClick={() => requestStartReporting(match.id, 2)}
                  disabled={isSubmitting}
                  className="rounded-xl border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800 px-4 py-4 text-sm font-semibold text-gray-900 dark:text-white transition hover:border-amber-400 hover:bg-amber-50 dark:hover:bg-amber-400/10 disabled:opacity-60"
                >
                  {match.t2_name}
                </button>
              </div>
              {errorMessage && (
                <div className="mt-4 rounded-xl border border-red-300 dark:border-red-500/40 bg-red-50 dark:bg-red-500/10 px-4 py-2">
                  <Text className="text-sm text-red-700 dark:text-red-200">{errorMessage}</Text>
                </div>
              )}
            </div>
          </div>
        </div>
      );
    }
  }

  // MATCH LIST VIEW
  const reporterHeading = isPoc ? 'Team Reporting' : 'Match Reporting';

  return (
    <div className="min-h-screen px-4 py-4 md:px-0">
      <div className="mx-auto max-w-3xl">
        <div className="rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-5">
          <div className="mb-4 flex items-center justify-between">
            <div>
              <Text as="h1" className="text-xl font-semibold text-gray-900 dark:text-white">{reporterHeading}</Text>
              <Text className="text-sm text-gray-500 dark:text-slate-400 mt-0.5">
                {isPoc ? 'Your team matches' : 'Upcoming and live matches'}
              </Text>
            </div>
            <div className="inline-flex items-center gap-1.5 rounded-full bg-gray-100 dark:bg-slate-800 px-3 py-1 text-xs font-semibold tracking-wider text-gray-500 dark:text-slate-400 uppercase">
              <Shield className="h-3 w-3" /> Volunteer
            </div>
          </div>

          {errorMessage && (
            <div className="mb-3 rounded-xl border border-red-300 dark:border-red-500/40 bg-red-50 dark:bg-red-500/10 px-4 py-2">
              <Text className="text-sm text-red-700 dark:text-red-200">{errorMessage}</Text>
            </div>
          )}

          <div className="space-y-2">
            {matches.length === 0 && <Text className="text-sm text-gray-400 dark:text-slate-500">No matches available right now.</Text>}
            {matches.map(match => {
              const status = getMatchStatus(match);
              const isLive = status === 'live';
              const isUpcoming = status === 'upcoming';

              return (
                <div
                  key={match.id}
                  className="rounded-xl border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800 p-4"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div>
                      <div className="text-sm font-semibold text-gray-900 dark:text-white">{match.t1_name} vs {match.t2_name}</div>
                      <div className="text-xs text-gray-500 dark:text-slate-400 mt-0.5">{formatTime(match.time)} &bull; {match.field_name || 'Field pending'}</div>
                    </div>
                    <span className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-bold tracking-wider uppercase ${
                      isLive ? 'bg-red-100 dark:bg-red-500/10 text-red-600 dark:text-red-300'
                        : isUpcoming ? 'bg-amber-100 dark:bg-amber-400/10 text-amber-700 dark:text-amber-300'
                        : 'bg-gray-200 dark:bg-slate-700 text-gray-500 dark:text-slate-400'
                    }`}>
                      <Circle className={`h-2 w-2 ${isLive ? 'fill-red-500 text-red-500' : isUpcoming ? 'fill-amber-500 text-amber-500' : 'fill-gray-400 text-gray-400'}`} />
                      {status}
                    </span>
                  </div>

                  <div className="mt-3 flex gap-2">
                    {isLive && (
                      <button
                        onClick={() => {
                          setActiveMatchId(match.id);
                          setPanel(match.possession === 1 ? 't1' : 't2');
                        }}
                        className="rounded-lg bg-amber-400 px-4 py-2 text-sm font-semibold text-gray-900 transition hover:bg-amber-300"
                      >
                        Resume Reporting
                      </button>
                    )}
                    {isUpcoming && (
                      <button
                        onClick={() => {
                          if (match.possession !== null) {
                            setActiveMatchId(match.id);
                          } else {
                            setChoosingPossession(match.id);
                          }
                        }}
                        className="rounded-lg bg-amber-400 px-4 py-2 text-sm font-semibold text-gray-900 transition hover:bg-amber-300"
                      >
                        Start Reporting
                      </button>
                    )}
                    {status === 'ended' && (
                      <button
                        onClick={() => setActiveMatchId(match.id)}
                        className="rounded-lg border border-gray-200 dark:border-slate-600 px-4 py-2 text-sm text-gray-700 dark:text-slate-200 transition hover:bg-gray-100 dark:hover:bg-slate-700"
                      >
                        View Match
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

export default function AdminPage() {
  return (
    <Suspense fallback={<div className="py-4 px-3 min-h-screen"><Text variant="primary">Loading...</Text></div>}>
      <AdminContent />
    </Suspense>
  );
}
