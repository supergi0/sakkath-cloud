'use client';

import { useEffect, useState, Suspense } from "react";
import { useSearchParams, useRouter } from "next/navigation";
import { ArrowLeft, Circle, Play } from "lucide-react";
import { Text } from "../components/Text";
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Legend, ReferenceDot } from 'recharts';
import useSWR from 'swr';

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

const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:9000';

type MatchTabType = 'log' | 'chart' | 'stats';

const fetcher = (url: string) => fetch(url).then(r => r.ok ? r.json() : null);

function MatchContent() {
  const searchParams = useSearchParams();
  const router = useRouter();
  const [activeTab, setActiveTab] = useState<MatchTabType>('log');
  const matchId = searchParams.get('match_id');

  // Determine if match is live to set refresh interval
  const { data: match, error, isLoading } = useSWR<MatchDetail>(
    matchId ? `${API_URL}/v1/matches/${matchId}` : null,
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
    return <div className="py-4 px-4 md:px-0 min-h-screen"><div className="max-w-7xl mx-auto"><Text variant="primary">Loading...</Text></div></div>;
  }

  if (!matchId || !match) {
    return <div className="py-4 px-4 md:px-0 min-h-screen"><div className="max-w-7xl mx-auto"><Text variant="primary">Match not found</Text></div></div>;
  }

  const status = getStatus();
  const isLive = status === 'live';
  const isEnded = status === 'ended';
  const isUpcoming = status === 'upcoming';
  const division = getDivision();
  const playerStats = getPlayerStats();
  const chartData = buildChartData();
  const matchDate = match.time ? new Date(match.time) : null;

  return (
    <div className="py-4 px-4 md:px-0 min-h-screen overflow-x-hidden">
      <div className="max-w-7xl mx-auto">
        <button 
          onClick={() => router.back()} 
          className="flex items-center gap-2 mb-4 text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100"
        >
          <ArrowLeft className="w-4 h-4" />
          <Text variant="secondary">Back</Text>
        </button>

        {/* Match Header */}
        <div className="rounded-sm p-4 md:p-6 bg-white dark:bg-slate-900 mb-4">
          {/* Top metadata row */}
          <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-2 mb-6 pb-4 border-b border-gray-200 dark:border-slate-700">
            <div className="flex flex-wrap items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
              {matchDate && (
                <>
                  <span>{matchDate.toLocaleDateString('en-US', { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' })}</span>
                  <span>•</span>
                  <span>{matchDate.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false })}</span>
                </>
              )}
              <span>•</span>
              <span>{match.field_name}</span>
              <span>•</span>
              <span>{division}</span>
              {match.stream_url && (
                <>
                  <span>•</span>
                  <a href={match.stream_url} target="_blank" rel="noopener noreferrer" className="inline-flex items-center gap-1 text-blue-500 hover:text-blue-400">
                    <Play className="w-3 h-3" /> Watch Game
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

          {/* Score display */}
          <div className="grid grid-cols-3 items-center gap-4">
            <div className="text-center">
              <div className="w-12 h-12 md:w-16 md:h-16 mx-auto mb-2 rounded-full bg-gray-100 dark:bg-slate-800 flex items-center justify-center overflow-hidden">
                {match.t1_small_logo ? (
                  <img src={match.t1_small_logo} alt={match.t1_name} className="w-full h-full object-cover" />
                ) : (
                  <span className="text-lg md:text-2xl font-bold text-gray-500">{match.t1_name.charAt(0)}</span>
                )}
              </div>
              <div 
                className={`text-sm md:text-base font-medium cursor-pointer hover:text-blue-500 ${isLive && match.possession === 1 ? 'text-blue-500' : 'text-gray-900 dark:text-gray-100'}`}
                onClick={() => router.push(`/teams?team_id=${match.t1_id}`)}
              >
                {match.t1_name}
              </div>
              <div className="text-3xl md:text-4xl font-bold text-gray-900 dark:text-white">
                {isUpcoming ? '-' : match.t1_score}
              </div>
              {isEnded && match.t1_spirit !== null && (
                <div className="text-sm text-gray-500 dark:text-gray-400 mt-2">Spirit: {match.t1_spirit}</div>
              )}
            </div>

            <div className="text-center">
              <div className="text-2xl text-gray-400">vs</div>
              {isLive && match.possession !== null && (
                <div className="text-xs text-gray-500 mt-2">
                  <span className="text-blue-500">{match.possession === 1 ? match.t1_name : match.t2_name}</span>
                </div>
              )}
            </div>

            <div className="text-center">
              <div className="w-12 h-12 md:w-16 md:h-16 mx-auto mb-2 rounded-full bg-gray-100 dark:bg-slate-800 flex items-center justify-center overflow-hidden">
                {match.t2_small_logo ? (
                  <img src={match.t2_small_logo} alt={match.t2_name} className="w-full h-full object-cover" />
                ) : (
                  <span className="text-lg md:text-2xl font-bold text-gray-500">{match.t2_name.charAt(0)}</span>
                )}
              </div>
              <div 
                className={`text-sm md:text-base font-medium cursor-pointer hover:text-blue-500 ${isLive && match.possession === 2 ? 'text-blue-500' : 'text-gray-900 dark:text-gray-100'}`}
                onClick={() => router.push(`/teams?team_id=${match.t2_id}`)}
              >
                {match.t2_name}
              </div>
              <div className="text-3xl md:text-4xl font-bold text-gray-900 dark:text-white">
                {isUpcoming ? '-' : match.t2_score}
              </div>
              {isEnded && match.t2_spirit !== null && (
                <div className="text-sm text-gray-500 dark:text-gray-400 mt-2">Spirit: {match.t2_spirit}</div>
              )}
            </div>
          </div>
        </div>

        {/* Tabs */}
        {!isUpcoming && (
          <>
            <div className="flex gap-2 mb-4">
                {(['log', 'chart', 'stats'] as MatchTabType[]).map((tab) => (
                <button
                  key={tab}
                  onClick={() => setActiveTab(tab)}
                  className={`px-4 py-2 text-sm font-medium rounded transition-colors ${
                    activeTab === tab
                      ? 'bg-blue-900 text-white'
                      : 'bg-gray-200 dark:bg-slate-700 text-gray-600 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-slate-600'
                  }`}
                >
                  {tab === 'log' ? 'Log' : tab === 'chart' ? 'Timeline' : 'Player Stats'}
                </button>
              ))}
            </div>

            {/* Log Tab */}
            {activeTab === 'log' && (
              <div className="rounded-sm p-4 md:p-6 bg-white dark:bg-slate-900">
                <div className="space-y-2 max-h-[500px] overflow-y-auto">
                  {match.events.length === 0 && <Text variant="secondary">No events recorded</Text>}
                  {match.events.slice().reverse().map((event) => {
                    const isT1 = event.team_id === match.t1_id;
                    const eventTypes = ['Score', 'Assist', 'Defense', 'Turnover'];
                    const eventColors = ['text-green-500', 'text-blue-500', 'text-purple-500', 'text-yellow-500'];
                    const eventTime = new Date(event.created_at).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
                    
                    return (
                      <div key={event.id} className={`flex ${isT1 ? 'justify-start' : 'justify-end'}`}>
                        <div className={`max-w-[85%] p-2 px-3 rounded-lg ${isT1 ? 'bg-gray-100 dark:bg-slate-800' : 'bg-blue-900/10 dark:bg-blue-900/20'}`}>
                          <div className="flex items-center gap-2 text-sm">
                            <span className="text-gray-500 dark:text-gray-400 text-xs">{eventTime}</span>
                            <span className={`font-medium ${eventColors[event.event_type]}`}>{eventTypes[event.event_type]}</span>
                            <span className="text-gray-900 dark:text-white">{event.player_name || 'Unknown'}</span>
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}

            {/* Chart Tab */}
            {activeTab === 'chart' && (
              <div className="rounded-sm p-4 md:p-6 bg-white dark:bg-slate-900">
                {chartData.length <= 1 ? (
                  <Text variant="secondary">No score data available</Text>
                ) : (
                  <div className="w-full overflow-x-auto overflow-y-hidden">
                    <div style={{ width: `${Math.max(1200, chartData.length * 40)}px`, height: '500px' }}>
                      <ResponsiveContainer width="100%" height="100%">
                        <LineChart data={chartData} margin={{ top: 30, right: 30, left: 10, bottom: 80 }}>
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
                    </ResponsiveContainer>
                  </div>
                  </div>
                )}
              </div>
            )}

            {/* Stats Tab */}
            {activeTab === 'stats' && (
              <div className="rounded-sm p-4 md:p-6 bg-white dark:bg-slate-900">
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
                          <td className="py-3 px-2">
                            <span className={`text-xs px-2 py-0.5 rounded ${player.team_id === match.t1_id ? 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300' : 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300'}`}>
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
          </>
        )}

        {isUpcoming && (
          <div className="rounded-sm p-4 md:p-6 bg-white dark:bg-slate-900">
            <Text variant="secondary">This match has not started yet.</Text>
          </div>
        )}
      </div>
    </div>
  );
}

export default function MatchesPage() {
  return (
    <Suspense fallback={<div className="py-4 px-4 min-h-screen"><div className="max-w-7xl mx-auto"><Text variant="primary">Loading...</Text></div></div>}>
      <MatchContent />
    </Suspense>
  );
}
