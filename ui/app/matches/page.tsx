'use client';

import { useEffect, useRef, useState, Suspense } from "react";
import { useSearchParams, useRouter } from "next/navigation";
import { ArrowLeft, ChevronDown, ChevronUp, Circle, Play } from "lucide-react";
import { Text } from "../components/Text";
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, Legend } from 'recharts';
import useSWR from 'swr';
import { apiUrl } from '../lib/api';
import { abbreviatePlayerName } from '../lib/player-name';
import { formatIndiaLongDate, formatIndiaTime } from '../lib/time';
import { subscribeToLiveUpdates } from '../lib/live-updates';
import { getTeamAbbreviation } from '../lib/team-name';

interface MatchDetail {
  id: number;
  t1_id: number;
  t2_id: number;
  t1_name: string;
  t2_name: string;
  t1_abbreviation: string | null;
  t2_abbreviation: string | null;
  t1_score: number;
  t2_score: number;
  t1_spirit: number | null;
  t2_spirit: number | null;
  t1_division: number;
  t2_division: number;
  t1_small_logo: string | null;
  t2_small_logo: string | null;
  possession: number | null;
  field_name: string;
  time: string;
  stream_url: string | null;
  started_at?: string | null;
  updated_at?: string;
  server_time?: string;
  players: { id: number; name: string; common_name: string | null; team_id: number }[];
  events: { id: number; player_id: number | null; player_name: string; team_id: number; event_type: number; created_at: string }[];
}

interface PlayerMatchStat {
  id: number;
  name: string;
  common_name: string | null;
  team_id: number;
  goals: number;
  assists: number;
  blocks: number;
  turnovers: number;
}

interface ChartDataPoint {
  time: string;
  minutes: number;
  t1: number;
  t2: number;
  event?: string;
}

interface SpiritScoreRow {
  id: number;
  match_id: number;
  team_id: number;
  rules_knowledge: number;
  fouls_contact: number;
  fair_mindedness: number;
  positive_attitude: number;
  communication: number;
  total: number;
  mvp_player_id: number | null;
  msp_player_id: number | null;
  notes: string | null;
  submitted_by_team_id: number;
}

type MatchTabType = 'log' | 'chart' | 'stats' | 'spirit';
type MatchStatsSortField = 'total' | 'goals' | 'assists' | 'blocks' | 'turnovers';
interface MatchesPreferences {
  activeTab: MatchTabType;
}

const MATCHES_PREFS_KEY = 'sakkath:matches:preferences';

function getTotalStat(player: Pick<PlayerMatchStat, 'goals' | 'assists' | 'blocks' | 'turnovers'>) {
  return player.goals + player.assists + player.blocks - player.turnovers;
}

const fetcher = (url: string) => fetch(url).then(r => r.ok ? r.json() : null);

function truncateLabel(value: string, maxLength: number) {
  if (value.length <= maxLength) return value;
  return `${value.slice(0, maxLength - 3)}...`;
}

function getDisplayMatchTeamName(name: string, abbreviation?: string | null) {
  if (!name.trim()) return name;
  if (name.length > 15) {
    return getTeamAbbreviation(name, abbreviation ?? undefined, 15);
  }
  return name;
}

function formatElapsed(elapsedSeconds: number) {
  const minutes = Math.floor(elapsedSeconds / 60);
  const seconds = elapsedSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

function getElapsedSeconds(startedAt: string, referenceTime?: string) {
  const startedMs = new Date(startedAt).getTime();
  const referenceMs = referenceTime ? new Date(referenceTime).getTime() : Date.now();
  if (Number.isNaN(startedMs) || Number.isNaN(referenceMs)) {
    return 0;
  }

  return Math.max(0, Math.floor((referenceMs - startedMs) / 1000));
}

function formatMatchDuration(startedAt?: string | null, updatedAt?: string) {
  if (!startedAt || !updatedAt) return null;
  return formatElapsed(getElapsedSeconds(startedAt, updatedAt));
}

function MatchTimer({ startedAt, serverTime }: { startedAt?: string | null; serverTime?: string }) {
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const initializedRef = useRef(false);

  useEffect(() => {
    if (!startedAt) {
      initializedRef.current = false;
      setElapsedSeconds(0);
      return;
    }

    const nextElapsed = getElapsedSeconds(startedAt, serverTime);
    setElapsedSeconds((current) => {
      if (!initializedRef.current) {
        initializedRef.current = true;
        return nextElapsed;
      }

      return Math.abs(current - nextElapsed) > 5 ? nextElapsed : current;
    });
  }, [startedAt, serverTime]);

  useEffect(() => {
    if (!startedAt) return;
    const interval = window.setInterval(() => {
      setElapsedSeconds((current) => current + 1);
    }, 1000);

    return () => window.clearInterval(interval);
  }, [startedAt]);

  if (!startedAt) return null;

  return <span className="font-mono text-sm font-semibold text-gray-500 dark:text-slate-400">{formatElapsed(elapsedSeconds)}</span>;
}

const SPIRIT_CRITERIA = [
  { key: 'rules_knowledge' as const, label: 'Rules Knowledge & Use' },
  { key: 'fouls_contact' as const, label: 'Fouls & Body Contact' },
  { key: 'fair_mindedness' as const, label: 'Fair-Mindedness' },
  { key: 'positive_attitude' as const, label: 'Positive Attitude & Self-Control' },
  { key: 'communication' as const, label: 'Communication' },
];

function MatchContent() {
  const searchParams = useSearchParams();
  const router = useRouter();
  const [activeTab, setActiveTab] = useState<MatchTabType>('log');
  const [statsSortField, setStatsSortField] = useState<MatchStatsSortField>('total');
  const [statsSortDir, setStatsSortDir] = useState<'asc' | 'desc'>('desc');
  const matchId = searchParams.get('match_id');
  const [showingCommonNames, setShowingCommonNames] = useState<Record<number, boolean>>({});

  const togglePlayerName = (id: number) => {
    setShowingCommonNames(prev => ({ ...prev, [id]: !prev[id] }));
  };

  useEffect(() => {
    const savedPreferences = localStorage.getItem(MATCHES_PREFS_KEY);
    if (savedPreferences) {
      try {
        const parsed: MatchesPreferences = JSON.parse(savedPreferences);
        if (['log', 'chart', 'stats', 'spirit'].includes(parsed.activeTab)) {
          setActiveTab(parsed.activeTab);
        }
      } catch {
      }
    }
  }, []);

  useEffect(() => {
    const preferences: MatchesPreferences = { activeTab };
    localStorage.setItem(MATCHES_PREFS_KEY, JSON.stringify(preferences));
  }, [activeTab]);

  const { data: match, mutate: mutateMatch, isLoading } = useSWR<MatchDetail>(
    matchId ? apiUrl(`/v1/matches/${matchId}`) : null,
    fetcher,
    {
      revalidateOnFocus: true,
      dedupingInterval: 5000,
    }
  );

  const { data: spirits = [], mutate: mutateSpirits } = useSWR<SpiritScoreRow[]>(
    matchId ? apiUrl(`/v1/matches/${matchId}/spirits`) : null,
    fetcher,
    {
      revalidateOnFocus: true,
      dedupingInterval: 5000,
    }
  );

  useEffect(() => {
    if (!matchId) return;
    const numericMatchId = Number(matchId);
    if (Number.isNaN(numericMatchId)) return;

    return subscribeToLiveUpdates({
      matchId: numericMatchId,
      onMatchUpdated: () => {
        void mutateMatch();
        void mutateSpirits();
      },
    });
  }, [matchId, mutateMatch, mutateSpirits]);

  const loading = isLoading;

  const getStatus = () => {
    if (!match) return '';
    if (match.possession === null) return 'upcoming';
    if (match.possession >= 3) return 'ended';
    return 'live';
  };

  const getDivision = () => {
    if (!match) return '';
    if (match.t1_division === match.t2_division) {
      return match.t1_division === 0 ? 'Open' : 'Women';
    }
    return 'Mixed';
  };

  const getPlayerStats = (): PlayerMatchStat[] => {
    if (!match) return [];
    const statsMap = new Map<number, PlayerMatchStat>();
    match.players.forEach(p => {
      statsMap.set(p.id, { id: p.id, name: p.name, common_name: p.common_name ?? null, team_id: p.team_id, goals: 0, assists: 0, blocks: 0, turnovers: 0 });
    });
    match.events.forEach(e => {
      if (!e.player_id) return; // Skip events without player
      const stat = statsMap.get(e.player_id);
      if (stat) {
        if (e.event_type === 0) stat.goals++;
        else if (e.event_type === 1) stat.assists++;
        else if (e.event_type === 2) stat.blocks++;
        else if (e.event_type === 3) stat.turnovers++;
      }
    });
    return Array.from(statsMap.values()).sort((a, b) => {
      const diff = (statsSortField === 'total' ? getTotalStat(a) : a[statsSortField]) - (statsSortField === 'total' ? getTotalStat(b) : b[statsSortField]);
      return statsSortDir === 'asc' ? diff : -diff;
    });
  };

  const buildChartData = (): ChartDataPoint[] => {
    if (!match) return [];
    const sortedEvents = [...match.events].sort((a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime());
    const data: ChartDataPoint[] = [{ time: '0:00', minutes: 0, t1: 0, t2: 0 }];
    let t1 = 0, t2 = 0;
    const startTime = sortedEvents.length > 0 ? new Date(sortedEvents[0].created_at).getTime() : 0;
    
    sortedEvents.forEach(event => {
      if (event.event_type === 0) {
        const mins = Math.round((new Date(event.created_at).getTime() - startTime) / 60000);
        if (event.team_id === match.t1_id) t1++;
        else t2++;
        data.push({ 
          time: `${mins}:00`, 
          minutes: mins, 
          t1, 
          t2, 
          event: `${event.player_name} scored` 
        });
      }
    });
    return data;
  };

  if (loading) {
    return <div className="py-4 px-3 md:px-0 min-h-screen"><div className="max-w-2xl mx-auto"><Text variant="primary">Loading...</Text></div></div>;
  }

  if (!matchId || !match) {
    return <div className="py-4 px-3 md:px-0 min-h-screen"><div className="max-w-2xl mx-auto"><Text variant="primary">Match not found</Text></div></div>;
  }

  const status = getStatus();
  const isLive = status === 'live';
  const isEnded = status === 'ended';
  const isUpcoming = status === 'upcoming';
  const division = getDivision();
  const playerStats = getPlayerStats();
  const chartData = buildChartData();
  const matchDate = match.time ? new Date(match.time) : null;
  const matchDuration = formatMatchDuration(match.started_at, match.updated_at);
  const displayT1Name = getDisplayMatchTeamName(match.t1_name, match.t1_abbreviation);
  const displayT2Name = getDisplayMatchTeamName(match.t2_name, match.t2_abbreviation);

  // Spirit tab data
  const resolvePlayer = (id: number | null) => !id ? null : match.players.find(p => p.id === id);
  const oppSpirits = spirits.filter(s => s.team_id !== s.submitted_by_team_id);
  const selfSpirits = spirits.filter(s => s.team_id === s.submitted_by_team_id);
  const t1OppSpirit = oppSpirits.find(s => s.team_id === match.t1_id);
  const t2OppSpirit = oppSpirits.find(s => s.team_id === match.t2_id);
  const t1SelfSpirit = selfSpirits.find(s => s.team_id === match.t1_id);
  const t2SelfSpirit = selfSpirits.find(s => s.team_id === match.t2_id);
  // MVP/MSP are nominated by the opposing team only
  const mvps = oppSpirits.filter(s => s.mvp_player_id).map(s => ({ player: resolvePlayer(s.mvp_player_id), teamId: resolvePlayer(s.mvp_player_id)?.team_id })).filter(m => m.player);
  const msps = oppSpirits.filter(s => s.msp_player_id).map(s => ({ player: resolvePlayer(s.msp_player_id), teamId: resolvePlayer(s.msp_player_id)?.team_id })).filter(m => m.player);
  const notes = oppSpirits
    .map(s => ({
      teamId: s.team_id,
      note: s.notes?.trim() ?? '',
    }))
    .filter((entry) => entry.note.length > 0);

  return (
    <div className="py-4 px-3 md:px-0 min-h-screen overflow-x-hidden">
      <div className="max-w-2xl mx-auto">
        <div className="mb-4 flex items-center justify-between gap-3">
          <button 
            onClick={() => router.back()} 
            className="flex items-center gap-2 text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100"
          >
            <ArrowLeft className="w-4 h-4" />
            <Text variant="secondary">Back</Text>
          </button>
          {isLive && match.started_at && (
            <MatchTimer startedAt={match.started_at} serverTime={match.server_time} />
          )}
          {isEnded && matchDuration && (
            <span className="font-mono text-sm font-semibold text-gray-500 dark:text-slate-400">{matchDuration}</span>
          )}
        </div>

        {/* Match Header */}
        <div className="rounded-2xl border border-gray-200 dark:border-slate-800 p-4 md:p-6 bg-white dark:bg-slate-900 mb-4">
          {/* Top metadata row */}
          <div className="mb-4 pb-3 border-b border-gray-200 dark:border-slate-700">
            <div className="flex flex-wrap items-center gap-2 text-sm text-gray-500 dark:text-gray-400 mb-1.5">
              {matchDate && (
                <>
                  <span>{formatIndiaLongDate(match.time)}</span>
                  <span>&bull;</span>
                  <span>{formatIndiaTime(match.time)}</span>
                </>
              )}
              <span>&bull;</span>
              <span>{match.field_name}</span>
              {match.stream_url && (
                <>
                  <span>&bull;</span>
                  <a href={match.stream_url} target="_blank" rel="noopener noreferrer" className="inline-flex items-center gap-1 text-blue-500 hover:text-blue-400">
                    <Play className="w-3 h-3" /> Watch
                  </a>
                </>
              )}
            </div>
            <div className="flex items-center justify-between text-sm">
              <span className="text-gray-500 dark:text-gray-400">{division}</span>
              <div className="flex items-center gap-2">
                {isLive && <><Circle className="w-2 h-2 fill-red-500 text-red-500" /><span className="font-medium text-red-500">Live</span></>}
                {isEnded && <><Circle className="w-2 h-2 fill-gray-400 text-gray-400" /><span className="text-gray-500">Ended</span></>}
                {isUpcoming && <><Circle className="w-2 h-2 fill-yellow-400 text-yellow-400" /><span className="font-medium text-yellow-500">Upcoming</span></>}
                {isEnded && matchDuration && <span className="font-mono text-gray-500 dark:text-slate-400">{matchDuration}</span>}
              </div>
            </div>
          </div>

          {/* Score display - matching admin layout */}
          <div className="grid grid-cols-[1fr_auto_1fr] items-center gap-2">
            <div
              onClick={() => router.push(`/teams?team_id=${match.t1_id}`)}
              className={`rounded-xl border p-3 md:p-4 text-left cursor-pointer transition hover:border-amber-400/50 overflow-hidden ${
                isLive && match.possession === 1 ? 'border-amber-400 bg-amber-50 dark:bg-amber-400/10' : 'border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800'
              }`}
            >
              <div className="flex items-center gap-2 mb-1">
                <div className="w-8 h-8 rounded-full bg-gray-200 dark:bg-slate-700 flex items-center justify-center overflow-hidden shrink-0">
                  {match.t1_small_logo ? (
                    <img src={match.t1_small_logo} alt={match.t1_name} className="w-full h-full object-cover" />
                  ) : (
                    <span className="text-sm font-bold text-gray-500">{match.t1_name.charAt(0)}</span>
                  )}
                </div>
                <span className="text-sm font-semibold text-gray-900 dark:text-white break-words leading-tight" title={match.t1_name}>{displayT1Name}</span>
              </div>
              {isLive && (
                <div className={`text-[10px] font-bold tracking-widest ${match.possession === 1 ? 'text-amber-600 dark:text-amber-400' : 'text-sky-600 dark:text-sky-400'}`}>
                  {match.possession === 1 ? 'OFFENSE' : 'DEFENSE'}
                </div>
              )}
              <div className="text-4xl font-black text-gray-900 dark:text-white mt-1">{isUpcoming ? '-' : match.t1_score}</div>
              {isEnded && match.t1_spirit !== null && match.t2_spirit !== null && (
                <div className="text-xs text-gray-500 dark:text-gray-400 mt-1">Spirit: {match.t1_spirit}</div>
              )}
            </div>

            <div className="text-lg text-gray-400 dark:text-slate-500 font-light">-</div>

            <div
              onClick={() => router.push(`/teams?team_id=${match.t2_id}`)}
              className={`rounded-xl border p-3 md:p-4 text-right cursor-pointer transition hover:border-amber-400/50 overflow-hidden ${
                isLive && match.possession === 2 ? 'border-amber-400 bg-amber-50 dark:bg-amber-400/10' : 'border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800'
              }`}
            >
              <div className="flex items-center gap-2 justify-end mb-1">
                <span className="text-sm font-semibold text-gray-900 dark:text-white break-words leading-tight" title={match.t2_name}>{displayT2Name}</span>
                <div className="w-8 h-8 rounded-full bg-gray-200 dark:bg-slate-700 flex items-center justify-center overflow-hidden shrink-0">
                  {match.t2_small_logo ? (
                    <img src={match.t2_small_logo} alt={match.t2_name} className="w-full h-full object-cover" />
                  ) : (
                    <span className="text-sm font-bold text-gray-500">{match.t2_name.charAt(0)}</span>
                  )}
                </div>
              </div>
              {isLive && (
                <div className={`text-[10px] font-bold tracking-widest ${match.possession === 2 ? 'text-amber-600 dark:text-amber-400' : 'text-sky-600 dark:text-sky-400'}`}>
                  {match.possession === 2 ? 'OFFENSE' : 'DEFENSE'}
                </div>
              )}
              <div className="text-4xl font-black text-gray-900 dark:text-white mt-1">{isUpcoming ? '-' : match.t2_score}</div>
              {isEnded && match.t1_spirit !== null && match.t2_spirit !== null && (
                <div className="text-xs text-gray-500 dark:text-gray-400 mt-1">Spirit: {match.t2_spirit}</div>
              )}
            </div>
          </div>
        </div>

        {/* Tabs */}
        {!isUpcoming && (
          <>
            <div className="grid grid-cols-4 rounded-full border border-gray-200 dark:border-slate-800 bg-gray-100 dark:bg-slate-900 p-1 mb-4">
              {(['log', 'chart', 'stats', 'spirit'] as MatchTabType[]).map((tab) => (
                <button
                  key={tab}
                  onClick={() => setActiveTab(tab)}
                  className={`truncate rounded-full px-2 py-2 text-sm font-semibold transition ${
                    activeTab === tab
                      ? 'bg-amber-400 text-gray-900 dark:text-slate-950'
                      : 'text-gray-600 dark:text-slate-400 hover:bg-gray-200 dark:hover:bg-slate-800'
                  }`}
                >
                  {tab === 'log' ? 'Log' : tab === 'chart' ? 'Graph' : tab === 'stats' ? 'Stats' : 'Spirit'}
                </button>
              ))}
            </div>

            {/* Log Tab */}
            {activeTab === 'log' && (
              <div className="rounded-2xl border border-gray-200 dark:border-slate-800 p-4 md:p-6 bg-white dark:bg-slate-900">
                <div className="space-y-1.5">
                  {match.events.length === 0 && <Text variant="secondary">No events recorded</Text>}
                  {match.events.slice().reverse().map((event) => {
                    const isT1 = event.team_id === match.t1_id;
                    const eventTypes = ['Score', 'Assist', 'Defense', 'Turnover'];
                    const eventColors = ['text-green-500', 'text-blue-500', 'text-purple-500', 'text-yellow-500'];
                    const eventTime = formatIndiaTime(event.created_at);
                    
                    return (
                      <div key={event.id} className={`flex ${isT1 ? 'justify-start' : 'justify-end'}`}>
                        <div className={`inline-flex items-center gap-2 rounded-lg px-3 py-2 ${isT1 ? 'bg-gray-100 dark:bg-slate-800' : 'bg-blue-900/10 dark:bg-blue-900/20'}`}>
                          <span className={`font-medium text-xs ${eventColors[event.event_type]}`}>{eventTypes[event.event_type]}</span>
                          <span className="text-sm text-gray-900 dark:text-white">{event.player_name || 'Unknown'}</span>
                          <span className="text-[10px] text-gray-500 dark:text-gray-400">{eventTime}</span>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}

            {/* Chart Tab */}
            {activeTab === 'chart' && (
              <div className="rounded-2xl border border-gray-200 dark:border-slate-800 p-4 md:p-6 bg-white dark:bg-slate-900">
                {chartData.length <= 1 ? (
                  <Text variant="secondary">No score data available</Text>
                ) : (
                  <div className="w-full overflow-x-auto overflow-y-hidden">
                    <div style={{ width: `${Math.max(600, chartData.length * 22)}px`, height: '500px' }}>
                      <LineChart data={chartData} width={Math.max(600, chartData.length * 22)} height={500} margin={{ top: 30, right: 30, left: 10, bottom: 80 }}>
                          <CartesianGrid strokeDasharray="3 3" stroke="#374151" opacity={0.3} />
                          <XAxis 
                            dataKey="time" 
                            stroke="#9ca3af" 
                            fontSize={12}
                            tick={{ fill: '#9ca3af' }}
                            label={{ value: 'Time', position: 'insideBottom', offset: -15, fill: '#9ca3af', fontWeight: 500 }}
                          />
                          <YAxis 
                            stroke="#9ca3af" 
                            fontSize={12}
                            tick={{ fill: '#9ca3af' }}
                            allowDecimals={false}
                            label={{ value: 'Score', angle: -90, position: 'insideLeft', fill: '#9ca3af', fontWeight: 500 }}
                          />
                        <Tooltip 
                          contentStyle={{ 
                            backgroundColor: '#1e293b', 
                            border: 'none', 
                            borderRadius: '8px',
                            color: '#fff',
                            fontSize: '12px',
                            padding: '8px'
                          }}
                          formatter={(value, name) => [value, name === 't1' ? match.t1_name : match.t2_name]}
                          labelFormatter={(label) => `Time: ${label}`}
                        />
                        <Legend 
                          formatter={(value) => value === 't1' ? match.t1_name : match.t2_name}
                          verticalAlign="bottom"
                          wrapperStyle={{ paddingTop: '40px', fontSize: '14px' }}
                        />
                        <Line 
                          type="stepAfter" 
                          dataKey="t1" 
                          stroke="#779ae6ff" 
                          strokeWidth={3}
                          dot={{ fill: '#2563eb', strokeWidth: 2, r: 4 }}
                          activeDot={{ r: 6, fill: '#2563eb' }}
                          animationDuration={1500}
                          animationEasing="ease-out"
                        />
                        <Line 
                          type="stepAfter" 
                          dataKey="t2" 
                          stroke="#d37979ff" 
                          strokeWidth={3}
                          dot={{ fill: '#dc2626', strokeWidth: 2, r: 4 }}
                          activeDot={{ r: 6, fill: '#dc2626' }}
                          animationDuration={1500}
                          animationEasing="ease-out"
                          animationBegin={200}
                        />
                      </LineChart>
                  </div>
                  </div>
                )}
              </div>
            )}

            {/* Stats Tab */}
            {activeTab === 'stats' && (
              <div className="rounded-2xl border border-gray-200 dark:border-slate-800 p-4 md:p-6 bg-white dark:bg-slate-900">
                <div className="overflow-x-auto">
                  <table className="w-full table-fixed text-sm">
                    <thead>
                      <tr className="border-b border-gray-200 dark:border-slate-700">
                        <th className="w-[168px] py-3 px-1.5 text-left font-medium text-gray-500 dark:text-gray-400">Player</th>
                        {([
                          ['total', 'Tot'],
                          ['goals', 'Gls'],
                          ['assists', 'Ast'],
                          ['blocks', 'Blk'],
                          ['turnovers', 'Tvr'],
                        ] as const).map(([field, label]) => {
                          const active = statsSortField === field;
                          return (
                            <th
                              key={field}
                              className={`w-12 py-3 px-1.5 font-medium cursor-pointer hover:text-gray-700 dark:hover:text-gray-200 ${active ? 'text-gray-900 dark:text-white' : 'text-gray-500 dark:text-gray-400'}`}
                              onClick={() => {
                                if (active) {
                                  setStatsSortDir((current) => current === 'asc' ? 'desc' : 'asc');
                                } else {
                                  setStatsSortField(field);
                                  setStatsSortDir('desc');
                                }
                              }}
                            >
                              <div className="flex items-center justify-center gap-1 whitespace-nowrap">
                                <span>{label}</span>
                                {active && (statsSortDir === 'asc' ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />)}
                              </div>
                            </th>
                          );
                        })}
                      </tr>
                    </thead>
                    <tbody>
                      {playerStats.length === 0 && (
                        <tr><td colSpan={6} className="py-4 text-center"><Text variant="secondary">No player data</Text></td></tr>
                      )}
                      {playerStats.map((player) => {
                        const commonName = player.common_name?.trim();
                        const hasAlternateName = Boolean(commonName && commonName !== player.name);
                        const showingCommonName = Boolean(showingCommonNames[player.id]);
                        const teamName = player.team_id === match.t1_id ? match.t1_name : match.t2_name;
                        const teamAbbr = player.team_id === match.t1_id
                          ? getTeamAbbreviation(match.t1_name, match.t1_abbreviation ?? undefined)
                          : getTeamAbbreviation(match.t2_name, match.t2_abbreviation ?? undefined);
                        const teamLabel = teamAbbr || truncateLabel(teamName, 20);
                        const displayName = abbreviatePlayerName(player.name, 20);
                        const displayCommonName = commonName ? abbreviatePlayerName(commonName, 20) : commonName;
                        return (
                          <tr key={player.id} onClick={() => togglePlayerName(player.id)} className="border-b border-gray-200 dark:border-slate-700 hover:bg-gray-50 dark:hover:bg-slate-800 cursor-pointer">
                            <td className="w-[168px] py-3 px-1.5">
                              <div className="flex flex-col justify-center">
                                {!hasAlternateName ? (
                                  <Text variant="primary" className="truncate font-medium" title={player.name}>{displayName}</Text>
                                ) : (
                                  <div className="relative inline-block pointer-events-none align-middle w-full">
                                    <span className="relative block h-[1.5rem] w-full overflow-hidden">
                                      <span className={`block truncate font-medium text-gray-900 transition-all duration-300 dark:text-white pointer-events-none ${showingCommonName ? '-translate-y-full scale-95 opacity-0' : 'translate-y-0 scale-100 opacity-100'}`} title={player.name}>
                                        {displayName}
                                      </span>
                                      <span className={`absolute inset-0 block truncate font-medium text-blue-700 transition-all duration-300 dark:text-cyan-300 pointer-events-none ${showingCommonName ? 'translate-y-0 scale-100 opacity-100' : 'translate-y-full scale-95 opacity-0'}`} title={commonName ?? undefined}>
                                        {displayCommonName}
                                      </span>
                                    </span>
                                  </div>
                                )}
                                <Text variant="secondary" className="text-[10px] leading-tight truncate" title={teamName}>{teamLabel}</Text>
                              </div>
                            </td>
                            <td className="w-12 py-3 px-1.5 text-center"><Text variant="primary">{getTotalStat(player)}</Text></td>
                            <td className="w-12 py-3 px-1.5 text-center"><Text variant="primary">{player.goals}</Text></td>
                            <td className="w-12 py-3 px-1.5 text-center"><Text variant="primary">{player.assists}</Text></td>
                            <td className="w-12 py-3 px-1.5 text-center"><Text variant="primary">{player.blocks}</Text></td>
                            <td className="w-12 py-3 px-1.5 text-center"><Text variant="primary">{player.turnovers}</Text></td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              </div>
            )}

            {/* Spirit Tab */}
            {activeTab === 'spirit' && (
              <div className="rounded-2xl border border-gray-200 dark:border-slate-800 p-4 md:p-6 bg-white dark:bg-slate-900">
                {(() => {
                  const t1Submitted = spirits.some(s => s.submitted_by_team_id === match.t1_id);
                  const t2Submitted = spirits.some(s => s.submitted_by_team_id === match.t2_id);
                  const bothSubmitted = t1Submitted && t2Submitted;

                  if (spirits.length === 0) {
                    return <Text variant="secondary">No spirit scores submitted yet</Text>;
                  }

                  if (!bothSubmitted) {
                    return (
                      <div>
                        <Text variant="primary" className="font-semibold mb-3 block">Spirit Scores</Text>
                        <div className="mt-2">
                          {[
                            { name: match.t1_name, submitted: t1Submitted },
                            { name: match.t2_name, submitted: t2Submitted },
                          ].map(({ name, submitted }) => (
                            <div key={name} className="flex items-center justify-between py-2.5 border-b border-gray-100 dark:border-slate-800 last:border-0">
                              <Text variant="primary">{name}</Text>
                              {submitted ? (
                                <span className="text-xs px-2 py-0.5 rounded bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400 font-medium">Submitted</span>
                              ) : (
                                <span className="text-xs px-2 py-0.5 rounded bg-gray-100 dark:bg-slate-700 text-gray-500 dark:text-gray-400">Yet to submit</span>
                              )}
                            </div>
                          ))}
                        </div>
                      </div>
                    );
                  }

                  return (
                    <>
                    {(t1OppSpirit || t2OppSpirit) && (
                      <div className="mb-6">
                        <Text variant="primary" className="text-center font-semibold text-blue-600 dark:text-blue-400 mb-3">Spirit Scores</Text>
                        <table className="w-full text-sm">
                          <thead>
                            <tr className="border-b border-gray-200 dark:border-slate-700">
                              <th className="py-2 px-2 text-left font-medium text-gray-500 dark:text-gray-400 text-xs uppercase tracking-wide">Criteria</th>
                              <th className="py-2 px-2 text-center font-medium text-gray-500 dark:text-gray-400 text-xs uppercase tracking-wide">{getTeamAbbreviation(match.t1_name, match.t1_abbreviation ?? undefined)}</th>
                              <th className="py-2 px-2 text-center font-medium text-gray-500 dark:text-gray-400 text-xs uppercase tracking-wide">{getTeamAbbreviation(match.t2_name, match.t2_abbreviation ?? undefined)}</th>
                            </tr>
                          </thead>
                          <tbody>
                            {SPIRIT_CRITERIA.map(c => (
                              <tr key={c.key} className="border-b border-gray-100 dark:border-slate-800">
                                <td className="py-2.5 px-2"><Text variant="primary" className="text-sm">{c.label}</Text></td>
                                <td className="py-2.5 px-2 text-center"><Text variant="primary">{t1OppSpirit ? t1OppSpirit[c.key] : '-'}</Text></td>
                                <td className="py-2.5 px-2 text-center"><Text variant="primary">{t2OppSpirit ? t2OppSpirit[c.key] : '-'}</Text></td>
                              </tr>
                            ))}
                            <tr className="border-t border-gray-300 dark:border-slate-600">
                              <td className="py-2.5 px-2"><Text variant="primary" className="font-semibold">Total</Text></td>
                              <td className="py-2.5 px-2 text-center"><Text variant="primary" className="font-semibold">{t1OppSpirit ? t1OppSpirit.total : '-'}</Text></td>
                              <td className="py-2.5 px-2 text-center"><Text variant="primary" className="font-semibold">{t2OppSpirit ? t2OppSpirit.total : '-'}</Text></td>
                            </tr>
                          </tbody>
                        </table>
                      </div>
                    )}
                    {(t1SelfSpirit || t2SelfSpirit) && (
                      <div className="mb-6">
                        <Text variant="primary" className="text-center font-semibold text-blue-600 dark:text-blue-400 mb-3">Spirit Scores - Self</Text>
                        <table className="w-full text-sm">
                          <thead>
                            <tr className="border-b border-gray-200 dark:border-slate-700">
                              <th className="py-2 px-2 text-left font-medium text-gray-500 dark:text-gray-400 text-xs uppercase tracking-wide">Criteria</th>
                              <th className="py-2 px-2 text-center font-medium text-gray-500 dark:text-gray-400 text-xs uppercase tracking-wide">{getTeamAbbreviation(match.t1_name, match.t1_abbreviation ?? undefined)}</th>
                              <th className="py-2 px-2 text-center font-medium text-gray-500 dark:text-gray-400 text-xs uppercase tracking-wide">{getTeamAbbreviation(match.t2_name, match.t2_abbreviation ?? undefined)}</th>
                            </tr>
                          </thead>
                          <tbody>
                            {SPIRIT_CRITERIA.map(c => (
                              <tr key={c.key} className="border-b border-gray-100 dark:border-slate-800">
                                <td className="py-2.5 px-2"><Text variant="primary" className="text-sm">{c.label}</Text></td>
                                <td className="py-2.5 px-2 text-center"><Text variant="primary">{t1SelfSpirit ? t1SelfSpirit[c.key] : '-'}</Text></td>
                                <td className="py-2.5 px-2 text-center"><Text variant="primary">{t2SelfSpirit ? t2SelfSpirit[c.key] : '-'}</Text></td>
                              </tr>
                            ))}
                            <tr className="border-t border-gray-300 dark:border-slate-600">
                              <td className="py-2.5 px-2"><Text variant="primary" className="font-semibold">Total</Text></td>
                              <td className="py-2.5 px-2 text-center"><Text variant="primary" className="font-semibold">{t1SelfSpirit ? t1SelfSpirit.total : '-'}</Text></td>
                              <td className="py-2.5 px-2 text-center"><Text variant="primary" className="font-semibold">{t2SelfSpirit ? t2SelfSpirit.total : '-'}</Text></td>
                            </tr>
                          </tbody>
                        </table>
                      </div>
                    )}
                    {mvps.length > 0 && (
                      <div className="mb-4">
                        <Text variant="primary" className="text-center font-semibold text-blue-600 dark:text-blue-400 mb-2">MVPs</Text>
                        <div className="space-y-1">
                          {mvps.map((m, i) => (
                            <div key={i} className="flex items-center gap-2">
                              <Text variant="primary" className="font-medium">{m.player!.name}</Text>
                              <span className={`text-xs px-2 py-0.5 rounded font-medium ${m.teamId === match.t1_id ? 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300' : 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300'}`}>
                                {m.teamId === match.t1_id ? getTeamAbbreviation(match.t1_name, match.t1_abbreviation ?? undefined) : getTeamAbbreviation(match.t2_name, match.t2_abbreviation ?? undefined)}
                              </span>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                    {msps.length > 0 && (
                      <div>
                        <Text variant="primary" className="text-center font-semibold text-blue-600 dark:text-blue-400 mb-2">MSPs</Text>
                        <div className="space-y-1">
                          {msps.map((m, i) => (
                            <div key={i} className="flex items-center gap-2">
                              <Text variant="primary" className="font-medium">{m.player!.name}</Text>
                              <span className={`text-xs px-2 py-0.5 rounded font-medium ${m.teamId === match.t1_id ? 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300' : 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300'}`}>
                                {m.teamId === match.t1_id ? getTeamAbbreviation(match.t1_name, match.t1_abbreviation ?? undefined) : getTeamAbbreviation(match.t2_name, match.t2_abbreviation ?? undefined)}
                              </span>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                    {notes.length > 0 && (
                      <div className={msps.length > 0 ? 'mt-4' : ''}>
                        <Text variant="primary" className="text-center font-semibold text-blue-600 dark:text-blue-400 mb-2">Notes</Text>
                        <div className="space-y-3">
                          {notes.map(({ teamId, note }) => (
                            <div key={teamId} className="rounded-xl border border-gray-200 bg-gray-50 px-3 py-2 dark:border-slate-700 dark:bg-slate-800">
                              <div className="mb-1">
                                <span className={`text-xs px-2 py-0.5 rounded font-medium ${teamId === match.t1_id ? 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300' : 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300'}`}>
                                  {teamId === match.t1_id ? getTeamAbbreviation(match.t1_name, match.t1_abbreviation ?? undefined) : getTeamAbbreviation(match.t2_name, match.t2_abbreviation ?? undefined)}
                                </span>
                              </div>
                              <Text variant="primary" className="whitespace-pre-wrap text-sm leading-6">{note}</Text>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                  </>
                  );
                })()}
              </div>
            )}
          </>
        )}

        {isUpcoming && (
          <div className="rounded-2xl border border-gray-200 dark:border-slate-800 p-4 md:p-6 bg-white dark:bg-slate-900">
            <Text variant="secondary">This match has not started yet.</Text>
          </div>
        )}
      </div>
    </div>
  );
}

export default function MatchesPage() {
  return (
    <Suspense fallback={<div className="py-4 px-3 min-h-screen"><div className="max-w-2xl mx-auto"><Text variant="primary">Loading...</Text></div></div>}>
      <MatchContent />
    </Suspense>
  );
}
