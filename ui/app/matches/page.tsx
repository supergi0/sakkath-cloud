'use client';

import { useEffect, useState, Suspense } from "react";
import { useSearchParams, useRouter } from "next/navigation";
import { ArrowLeft, Circle, Play } from "lucide-react";
import { Text } from "../components/Text";
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, Legend } from 'recharts';
import useSWR from 'swr';
import { apiUrl } from '../lib/api';

interface MatchDetail {
  id: number;
  t1_id: number;
  t2_id: number;
  t1_name: string;
  t2_name: string;
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
  players: { id: number; name: string; team_id: number }[];
  events: { id: number; player_id: number | null; player_name: string; team_id: number; event_type: number; created_at: string }[];
}

interface PlayerMatchStat {
  id: number;
  name: string;
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
  submitted_by_team_id: number;
}

type MatchTabType = 'log' | 'chart' | 'stats' | 'spirit';
interface MatchesPreferences {
  activeTab: MatchTabType;
}

const MATCHES_PREFS_KEY = 'sakkath:matches:preferences';

const fetcher = (url: string) => fetch(url).then(r => r.ok ? r.json() : null);

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
  const matchId = searchParams.get('match_id');

  const [spirits, setSpirits] = useState<SpiritScoreRow[]>([]);

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
    if (!matchId) return;
    fetch(apiUrl(`/v1/matches/${matchId}/spirits`))
      .then(r => r.ok ? r.json() : [])
      .then(setSpirits)
      .catch(() => {});
  }, [matchId]);

  useEffect(() => {
    const preferences: MatchesPreferences = { activeTab };
    localStorage.setItem(MATCHES_PREFS_KEY, JSON.stringify(preferences));
  }, [activeTab]);

  // Determine if match is live to set refresh interval
  const { data: match, error, isLoading } = useSWR<MatchDetail>(
    matchId ? apiUrl(`/v1/matches/${matchId}`) : null,
    fetcher,
    {
      refreshInterval: (data) => {
        // Refresh every 15s if live, otherwise no auto-refresh
        if (!data) return 0;
        const isLive = data.possession !== null && data.possession < 3;
        return isLive ? 15000 : 0;
      },
      revalidateOnFocus: true,
      dedupingInterval: 5000,
    }
  );

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
      statsMap.set(p.id, { id: p.id, name: p.name, team_id: p.team_id, goals: 0, assists: 0, blocks: 0, turnovers: 0 });
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
    return Array.from(statsMap.values()).sort((a, b) => (b.goals + b.assists + b.blocks) - (a.goals + a.assists + a.blocks));
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

  // Spirit tab data
  const resolvePlayer = (id: number | null) => !id ? null : match.players.find(p => p.id === id);
  const oppSpirits = spirits.filter(s => s.team_id !== s.submitted_by_team_id);
  const selfSpirits = spirits.filter(s => s.team_id === s.submitted_by_team_id);
  const t1OppSpirit = oppSpirits.find(s => s.team_id === match.t1_id);
  const t2OppSpirit = oppSpirits.find(s => s.team_id === match.t2_id);
  const t1SelfSpirit = selfSpirits.find(s => s.team_id === match.t1_id);
  const t2SelfSpirit = selfSpirits.find(s => s.team_id === match.t2_id);
  // MVP/MSP are nominated by the opposing team only
  const mvps = oppSpirits.filter(s => s.mvp_player_id).map(s => ({ player: resolvePlayer(s.mvp_player_id), byTeam: s.submitted_by_team_id === match.t1_id ? match.t1_name : match.t2_name })).filter(m => m.player);
  const msps = oppSpirits.filter(s => s.msp_player_id).map(s => ({ player: resolvePlayer(s.msp_player_id), byTeam: s.submitted_by_team_id === match.t1_id ? match.t1_name : match.t2_name })).filter(m => m.player);

  return (
    <div className="py-4 px-3 md:px-0 min-h-screen overflow-x-hidden">
      <div className="max-w-2xl mx-auto">
        <button 
          onClick={() => router.back()} 
          className="flex items-center gap-2 mb-4 text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100"
        >
          <ArrowLeft className="w-4 h-4" />
          <Text variant="secondary">Back</Text>
        </button>

        {/* Match Header */}
        <div className="rounded-2xl border border-gray-200 dark:border-slate-800 p-4 md:p-6 bg-white dark:bg-slate-900 mb-4">
          {/* Top metadata row */}
          <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-2 mb-4 pb-3 border-b border-gray-200 dark:border-slate-700">
            <div className="flex flex-wrap items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
              {matchDate && (
                <>
                  <span>{matchDate.toLocaleDateString('en-US', { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' })}</span>
                  <span>&bull;</span>
                  <span>{matchDate.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false })}</span>
                </>
              )}
              <span>&bull;</span>
              <span>{match.field_name}</span>
              <span>&bull;</span>
              <span>{division}</span>
              {match.stream_url && (
                <>
                  <span>&bull;</span>
                  <a href={match.stream_url} target="_blank" rel="noopener noreferrer" className="inline-flex items-center gap-1 text-blue-500 hover:text-blue-400">
                    <Play className="w-3 h-3" /> Watch
                  </a>
                </>
              )}
            </div>
            <div className="flex items-center gap-2">
              {isLive && <><Circle className="w-2 h-2 fill-red-500 text-red-500" /><span className="text-sm font-medium text-red-500">Live</span></>}
              {isEnded && <><Circle className="w-2 h-2 fill-gray-400 text-gray-400" /><span className="text-sm text-gray-500">Ended</span></>}
              {isUpcoming && <><Circle className="w-2 h-2 fill-yellow-400 text-yellow-400" /><span className="text-sm font-medium text-yellow-500">Upcoming</span></>}
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
                <span className="text-sm font-semibold text-gray-900 dark:text-white break-words leading-tight">{match.t1_name}</span>
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
                <span className="text-sm font-semibold text-gray-900 dark:text-white break-words leading-tight">{match.t2_name}</span>
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
                    const eventTime = new Date(event.created_at).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false });
                    
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
                    <div style={{ width: `${Math.max(1200, chartData.length * 40)}px`, height: '500px' }}>
                      <LineChart data={chartData} width={Math.max(1200, chartData.length * 40)} height={500} margin={{ top: 30, right: 30, left: 10, bottom: 80 }}>
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
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="border-b border-gray-200 dark:border-slate-700">
                        <th className="py-3 px-2 text-left font-medium text-gray-500 dark:text-gray-400">Player</th>
                        <th className="py-3 px-2 text-left font-medium text-gray-500 dark:text-gray-400">Team</th>
                        <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">G</th>
                        <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">A</th>
                        <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">D</th>
                        <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">T</th>
                      </tr>
                    </thead>
                    <tbody>
                      {playerStats.length === 0 && (
                        <tr><td colSpan={6} className="py-4 text-center"><Text variant="secondary">No player data</Text></td></tr>
                      )}
                      {playerStats.map((player) => (
                        <tr key={player.id} className="border-b border-gray-200 dark:border-slate-700 hover:bg-gray-50 dark:hover:bg-slate-800">
                          <td className="py-3 px-2"><Text variant="primary" className="font-medium">{player.name}</Text></td>
                          <td className="py-3 px-2 max-w-[100px]">
                            <span className={`text-xs px-2 py-0.5 rounded inline-block max-w-full truncate ${player.team_id === match.t1_id ? 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300' : 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300'}`}>
                              {player.team_id === match.t1_id ? match.t1_name : match.t2_name}
                            </span>
                          </td>
                          <td className="py-3 px-2 text-center"><Text variant="primary">{player.goals}</Text></td>
                          <td className="py-3 px-2 text-center"><Text variant="primary">{player.assists}</Text></td>
                          <td className="py-3 px-2 text-center"><Text variant="primary">{player.blocks}</Text></td>
                          <td className="py-3 px-2 text-center"><Text variant="primary">{player.turnovers}</Text></td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            )}

            {/* Spirit Tab */}
            {activeTab === 'spirit' && (
              <div className="rounded-2xl border border-gray-200 dark:border-slate-800 p-4 md:p-6 bg-white dark:bg-slate-900">
                {spirits.length === 0 ? (
                  <Text variant="secondary">No spirit scores submitted yet</Text>
                ) : (
                  <>
                    {(t1OppSpirit || t2OppSpirit) && (
                      <div className="mb-6">
                        <Text variant="primary" className="text-center font-semibold text-blue-600 dark:text-blue-400 mb-3">Spirit Scores</Text>
                        <table className="w-full text-sm">
                          <thead>
                            <tr className="border-b border-gray-200 dark:border-slate-700">
                              <th className="py-2 px-2 text-left font-medium text-gray-500 dark:text-gray-400 text-xs uppercase tracking-wide">Criteria</th>
                              <th className="py-2 px-2 text-center font-medium text-gray-500 dark:text-gray-400 text-xs uppercase tracking-wide truncate max-w-[100px]">{match.t1_name}</th>
                              <th className="py-2 px-2 text-center font-medium text-gray-500 dark:text-gray-400 text-xs uppercase tracking-wide truncate max-w-[100px]">{match.t2_name}</th>
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
                              <th className="py-2 px-2 text-center font-medium text-gray-500 dark:text-gray-400 text-xs uppercase tracking-wide truncate max-w-[100px]">{match.t1_name}</th>
                              <th className="py-2 px-2 text-center font-medium text-gray-500 dark:text-gray-400 text-xs uppercase tracking-wide truncate max-w-[100px]">{match.t2_name}</th>
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
                        {mvps.map((m, i) => (
                          <div key={i} className="mb-1">
                            <Text variant="primary" className="font-medium">{m.player!.name}</Text>
                            <Text variant="secondary" className="text-xs uppercase tracking-wide">{m.byTeam}</Text>
                          </div>
                        ))}
                      </div>
                    )}
                    {msps.length > 0 && (
                      <div>
                        <Text variant="primary" className="text-center font-semibold text-blue-600 dark:text-blue-400 mb-2">MSPs</Text>
                        {msps.map((m, i) => (
                          <div key={i} className="mb-1">
                            <Text variant="primary" className="font-medium">{m.player!.name}</Text>
                            <Text variant="secondary" className="text-xs uppercase tracking-wide">{m.byTeam}</Text>
                          </div>
                        ))}
                      </div>
                    )}
                  </>
                )}
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
