'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { Circle, Play, X } from 'lucide-react';
import useSWR from 'swr';
import { Text } from '../components/Text';
import { useAuth } from '../auth-provider';

interface UpcomingMatch {
  id: number;
  t1_id: number;
  t2_id: number;
  t1_name: string;
  t2_name: string;
  field_name: string;
  time: string;
  possession: number | null;
}

interface MatchDetail {
  id: number;
  t1_id: number;
  t2_id: number;
  t1_name: string;
  t2_name: string;
  t1_score: number;
  t2_score: number;
  possession: number | null;
  players: { id: number; name: string; team_id: number }[];
  events: {
    id: number;
    player_id: number | null;
    player_name: string;
    team_id: number;
    event_type: number;
    actor_user_id: number | null;
    created_at: string;
  }[];
}

interface SelectedPlayers {
  scorer: number | null;
  assister: number | null;
}

const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:9000';

export default function AdminPage() {
  const { isLoggedIn, isAdmin, token, isLoading } = useAuth();
  const router = useRouter();
  const [activeMatchId, setActiveMatchId] = useState<number | null>(null);
  const [popup, setPopup] = useState<'score' | 'turnover' | 'defense' | null>(null);
  const [selectedPlayers, setSelectedPlayers] = useState<SelectedPlayers>({ scorer: null, assister: null });
  const [startPossessionByMatch, setStartPossessionByMatch] = useState<Record<number, number>>({});

  useEffect(() => {
    if (isLoading) return;
    if (!isLoggedIn || !isAdmin) {
      router.push('/login');
    }
  }, [isLoggedIn, isAdmin, isLoading, router]);

  const fetchAdminMatches = async (): Promise<UpcomingMatch[]> => {
    if (!token) return [];
    const res = await fetch(`${API_URL}/v1/admin/matches`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) return [];
    return res.json();
  };

  const fetchMatchDetail = async (url: string): Promise<MatchDetail | null> => {
    const res = await fetch(url);
    if (!res.ok) return null;
    return res.json();
  };

  const { data: matches = [], mutate: mutateMatches, isLoading: matchesLoading } = useSWR(
    token && isLoggedIn && isAdmin ? `${API_URL}/v1/admin/matches` : null,
    fetchAdminMatches,
    {
      refreshInterval: 5000,
      revalidateOnFocus: true,
    }
  );

  const { data: activeMatch, mutate: mutateActiveMatch } = useSWR(
    activeMatchId ? `${API_URL}/v1/matches/${activeMatchId}` : null,
    fetchMatchDetail,
    {
      refreshInterval: 5000,
      revalidateOnFocus: true,
    }
  );

  const startReporting = async (matchId: number) => {
    if (!token) return;
    const possession = startPossessionByMatch[matchId] || 1;
    const res = await fetch(`${API_URL}/v1/admin/matches/${matchId}/start`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ possession }),
    });

    if (res.ok || res.status === 409) {
      setActiveMatchId(matchId);
      mutateMatches();
    }
  };

  const undoEvent = async () => {
    if (!activeMatch || !token) return;
    const res = await fetch(`${API_URL}/v1/admin/matches/${activeMatch.id}/undo`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${token}` },
    });
    if (res.ok) {
      mutateActiveMatch();
      mutateMatches();
    }
  };

  const recordScore = async () => {
    if (!activeMatch || !token) return;

    await fetch(`${API_URL}/v1/admin/matches/${activeMatch.id}/event`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ player_id: selectedPlayers.scorer, event_type: 0 }),
    });

    if (selectedPlayers.assister) {
      await fetch(`${API_URL}/v1/admin/matches/${activeMatch.id}/event`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify({ player_id: selectedPlayers.assister, event_type: 1 }),
      });
    }

    mutateActiveMatch();
    mutateMatches();
    setPopup(null);
    setSelectedPlayers({ scorer: null, assister: null });
  };

  const recordEvent = async (eventType: number, playerId: number | null) => {
    if (!activeMatch || !token) return;

    await fetch(`${API_URL}/v1/admin/matches/${activeMatch.id}/event`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ player_id: playerId, event_type: eventType }),
    });

    mutateActiveMatch();
    mutateMatches();
    setPopup(null);
    setSelectedPlayers({ scorer: null, assister: null });
  };

  const endMatch = async () => {
    if (!activeMatch || !token) return;

    await fetch(`${API_URL}/v1/admin/matches/${activeMatch.id}/end`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${token}` },
    });

    setActiveMatchId(null);
    mutateMatches();
  };

  const formatTime = (time: string) => {
    if (!time) return '';
    const d = new Date(time);
    return d.toLocaleDateString('en-US', { weekday: 'short', day: 'numeric', month: 'short' }) + ' - ' + d.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false });
  };

  const getMatchStatus = (match: UpcomingMatch) => {
    if (match.possession !== null && match.possession >= 3) return 'ended';
    if (match.possession !== null && match.possession <= 2) return 'live';
    return 'upcoming';
  };

  if (isLoading || matchesLoading) {
    return (
      <div className="py-4 px-4 md:px-0 min-h-screen">
        <div className="max-w-4xl mx-auto">
          <Text variant="primary">Loading...</Text>
        </div>
      </div>
    );
  }

  if (activeMatch && activeMatch.possession !== null && activeMatch.possession < 3) {
    const posTeamId = activeMatch.possession === 1 ? activeMatch.t1_id : activeMatch.t2_id;
    const defTeamId = activeMatch.possession === 1 ? activeMatch.t2_id : activeMatch.t1_id;
    const posTeam = activeMatch.possession === 1 ? activeMatch.t1_name : activeMatch.t2_name;
    const posPlayers = activeMatch.players.filter((p) => p.team_id === posTeamId);
    const defPlayers = activeMatch.players.filter((p) => p.team_id === defTeamId);

    return (
      <div className="py-4 px-4 md:px-0 min-h-screen overflow-x-hidden">
        <div className="max-w-4xl mx-auto">
          <div className="rounded-sm p-6 bg-white dark:bg-slate-900 mb-4">
            <div className="flex items-center justify-between mb-4">
              <Text as="h1" variant="primary" className="text-xl font-semibold">Live Reporting</Text>
              <button onClick={endMatch} className="px-4 py-2 bg-red-600 text-white rounded text-sm font-medium hover:bg-red-700">
                End Match
              </button>
            </div>

            <div className="flex items-center justify-center gap-8 mb-6">
              <div className="text-center">
                <Text variant="primary" className={`text-lg font-medium ${activeMatch.possession === 1 ? 'text-blue-500' : ''}`}>{activeMatch.t1_name}</Text>
                <Text variant="primary" className="text-4xl font-bold">{activeMatch.t1_score}</Text>
              </div>
              <Text variant="secondary" className="text-2xl">-</Text>
              <div className="text-center">
                <Text variant="primary" className={`text-lg font-medium ${activeMatch.possession === 2 ? 'text-blue-500' : ''}`}>{activeMatch.t2_name}</Text>
                <Text variant="primary" className="text-4xl font-bold">{activeMatch.t2_score}</Text>
              </div>
            </div>

            <div className="text-center mb-6">
              <Text variant="secondary" className="text-sm">Possession: <span className="text-blue-500 font-medium">{posTeam}</span></Text>
            </div>

            <div className="grid grid-cols-3 gap-3 mb-4">
              <button
                onClick={() => setPopup('score')}
                className="w-full px-4 py-3 bg-gray-200 dark:bg-slate-700 text-gray-900 dark:text-white rounded font-medium hover:bg-gray-300 dark:hover:bg-slate-600"
              >
                Score
              </button>
              <button
                onClick={() => setPopup('turnover')}
                className="w-full px-4 py-3 bg-gray-200 dark:bg-slate-700 text-gray-900 dark:text-white rounded font-medium hover:bg-gray-300 dark:hover:bg-slate-600"
              >
                Turnover
              </button>
              <button
                onClick={() => setPopup('defense')}
                className="w-full px-4 py-3 bg-gray-200 dark:bg-slate-700 text-gray-900 dark:text-white rounded font-medium hover:bg-gray-300 dark:hover:bg-slate-600"
              >
                Defense
              </button>
            </div>

            <div className="flex justify-center mb-6">
              <button
                onClick={undoEvent}
                className="px-4 py-2 bg-yellow-600 text-white rounded text-sm font-medium hover:bg-yellow-700"
              >
                Undo Last Event
              </button>
            </div>
          </div>

          <div className="rounded-sm p-6 bg-white dark:bg-slate-900">
            <Text as="h2" variant="primary" className="text-lg font-semibold mb-4">Live Log</Text>
            <div className="space-y-2 max-h-64 overflow-y-auto">
              {activeMatch.events.length === 0 && <Text variant="secondary">No events yet</Text>}
              {activeMatch.events.slice().reverse().map((event) => {
                const isT1 = event.team_id === activeMatch.t1_id;
                const eventTypes = ['Score', 'Assist', 'Defense', 'Turnover'];
                const eventColors = ['text-green-500', 'text-blue-500', 'text-purple-500', 'text-yellow-500'];
                const time = new Date(event.created_at).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
                const teamName = isT1 ? activeMatch.t1_name : activeMatch.t2_name;
                const displayName = event.player_name || `${teamName}`;

                return (
                  <div key={event.id} className={`flex ${isT1 ? 'justify-start' : 'justify-end'}`}>
                    <div className={`max-w-[80%] p-2 rounded ${isT1 ? 'bg-gray-100 dark:bg-slate-800' : 'bg-blue-900/20 dark:bg-blue-900/30'}`}>
                      <div className="flex items-center gap-2">
                        <Text variant="secondary" className="text-xs">{time}</Text>
                        <span className={`text-sm font-medium ${eventColors[event.event_type]}`}>
                          {eventTypes[event.event_type]}
                        </span>
                        <Text variant="primary" className="text-sm">{displayName}</Text>
                        {event.actor_user_id && (
                          <Text variant="secondary" className="text-xs">rec #{event.actor_user_id}</Text>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          {popup === 'score' && (
            <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onClick={() => setPopup(null)}>
              <div className="bg-white dark:bg-slate-900 rounded-lg p-6 w-80 max-h-[80vh] overflow-y-auto" onClick={(e) => e.stopPropagation()}>
                <div className="flex items-center justify-between mb-4">
                  <Text variant="primary" className="font-semibold">Record Score</Text>
                  <button onClick={() => setPopup(null)}><X className="w-5 h-5 text-gray-500" /></button>
                </div>
                <div className="space-y-4">
                  <div>
                    <Text variant="secondary" className="text-sm mb-2">Scorer</Text>
                    <select
                      className="w-full p-2 rounded border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-gray-900 dark:text-white"
                      value={selectedPlayers.scorer || ''}
                      onChange={(e) => setSelectedPlayers((p) => ({ ...p, scorer: e.target.value ? parseInt(e.target.value) : null }))}
                    >
                      <option value="">Select scorer (optional)</option>
                      {posPlayers.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
                    </select>
                  </div>
                  <div>
                    <Text variant="secondary" className="text-sm mb-2">Assister</Text>
                    <select
                      className="w-full p-2 rounded border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-gray-900 dark:text-white"
                      value={selectedPlayers.assister || ''}
                      onChange={(e) => setSelectedPlayers((p) => ({ ...p, assister: e.target.value ? parseInt(e.target.value) : null }))}
                    >
                      <option value="">Select assister (optional)</option>
                      {posPlayers.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
                    </select>
                  </div>
                  <button onClick={recordScore} className="w-full py-2 bg-blue-900 text-white rounded font-medium hover:bg-blue-800">
                    Confirm Score
                  </button>
                </div>
              </div>
            </div>
          )}

          {popup === 'turnover' && (
            <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onClick={() => setPopup(null)}>
              <div className="bg-white dark:bg-slate-900 rounded-lg p-6 w-80 max-h-[80vh] overflow-y-auto" onClick={(e) => e.stopPropagation()}>
                <div className="flex items-center justify-between mb-4">
                  <Text variant="primary" className="font-semibold">Record Turnover</Text>
                  <button onClick={() => setPopup(null)}><X className="w-5 h-5 text-gray-500" /></button>
                </div>
                <div className="space-y-4">
                  <select
                    className="w-full p-2 rounded border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-gray-900 dark:text-white"
                    value={selectedPlayers.scorer || ''}
                    onChange={(e) => setSelectedPlayers((p) => ({ ...p, scorer: e.target.value ? parseInt(e.target.value) : null }))}
                  >
                    <option value="">Select player (optional)</option>
                    {posPlayers.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
                  </select>
                  <button onClick={() => recordEvent(3, selectedPlayers.scorer)} className="w-full py-2 bg-blue-900 text-white rounded font-medium hover:bg-blue-800">
                    Confirm Turnover
                  </button>
                </div>
              </div>
            </div>
          )}

          {popup === 'defense' && (
            <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onClick={() => setPopup(null)}>
              <div className="bg-white dark:bg-slate-900 rounded-lg p-6 w-80 max-h-[80vh] overflow-y-auto" onClick={(e) => e.stopPropagation()}>
                <div className="flex items-center justify-between mb-4">
                  <Text variant="primary" className="font-semibold">Record Defense</Text>
                  <button onClick={() => setPopup(null)}><X className="w-5 h-5 text-gray-500" /></button>
                </div>
                <div className="space-y-4">
                  <select
                    className="w-full p-2 rounded border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-gray-900 dark:text-white"
                    value={selectedPlayers.scorer || ''}
                    onChange={(e) => setSelectedPlayers((p) => ({ ...p, scorer: e.target.value ? parseInt(e.target.value) : null }))}
                  >
                    <option value="">Select player (optional)</option>
                    {defPlayers.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
                  </select>
                  <button onClick={() => recordEvent(2, selectedPlayers.scorer)} className="w-full py-2 bg-blue-900 text-white rounded font-medium hover:bg-blue-800">
                    Confirm Defense
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="py-4 px-4 md:px-0 min-h-screen overflow-x-hidden">
      <div className="max-w-4xl mx-auto">
        <div className="rounded-sm p-6 bg-white dark:bg-slate-900">
          <Text as="h1" variant="primary" className="text-xl font-semibold mb-4">Matches</Text>

          <div className="space-y-3">
            {matches.length === 0 && <Text variant="secondary">No matches</Text>}
            {matches.map((match) => {
              const status = getMatchStatus(match);
              const isLive = status === 'live';
              const isEnded = status === 'ended';

              return (
                <div
                  key={match.id}
                  className="p-4 rounded border bg-gray-50 dark:bg-slate-800 border-gray-200 dark:border-slate-700"
                >
                  <div className="flex items-center justify-between mb-2">
                    <Text variant="secondary" className="text-xs">{formatTime(match.time)}</Text>
                    <div className="flex items-center gap-2">
                      <Text variant="secondary" className="text-xs">{match.field_name}</Text>
                      {isLive && <Circle className="w-3 h-3 fill-red-500 text-red-500" />}
                      {isEnded && <Circle className="w-3 h-3 fill-gray-400 text-gray-400" />}
                      {!isLive && !isEnded && <Circle className="w-3 h-3 fill-yellow-400 text-yellow-400" />}
                    </div>
                  </div>

                  <div className="mb-3">
                    <Text variant="primary" className="font-medium leading-snug">{`${match.t1_name} vs ${match.t2_name}`}</Text>
                  </div>

                  {!isEnded && (
                    <div className="flex gap-2">
                      {!isLive && (
                        <div className="flex flex-col gap-2 w-full">
                          <div className="flex gap-2 items-center">
                            <Text variant="secondary" className="text-xs">Starting possession:</Text>
                            <select
                              value={startPossessionByMatch[match.id] || 1}
                              onChange={(e) => setStartPossessionByMatch((prev) => ({ ...prev, [match.id]: parseInt(e.target.value) }))}
                              className="flex-1 p-1.5 text-sm rounded border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-gray-900 dark:text-white"
                            >
                              <option value={1}>{match.t1_name}</option>
                              <option value={2}>{match.t2_name}</option>
                            </select>
                          </div>
                          <button
                            onClick={() => startReporting(match.id)}
                            className="px-3 py-1.5 bg-blue-900 text-white rounded text-sm font-medium hover:bg-blue-800 flex items-center justify-center gap-1"
                          >
                            <Play className="w-4 h-4" /> Start Reporting
                          </button>
                        </div>
                      )}
                      {isLive && (
                        <button
                          onClick={() => setActiveMatchId(match.id)}
                          className="px-3 py-1.5 bg-red-600 text-white rounded text-sm font-medium hover:bg-red-700"
                        >
                          Continue Reporting
                        </button>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
