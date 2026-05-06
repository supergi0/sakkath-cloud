'use client';

import { useEffect, useState, Suspense } from "react";
import { useSearchParams, useRouter } from "next/navigation";
import { MapPin, Users, Trophy, Target, ArrowUp, ArrowDown, Star, TrendingUp, TrendingDown, Sparkle, AlignStartVertical, Play, Circle, ChevronUp, ChevronDown } from "lucide-react";
import { Text } from "../components/Text";
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip } from 'recharts';
import { apiUrl } from '../lib/api';
import { abbreviatePlayerName } from '../lib/player-name';
import { getTeamAbbreviation } from '../lib/team-name';
import { formatIndiaShortDateTime } from '../lib/time';

interface Team {
  id: number;
  name: string;
  abbreviation?: string | null;
  location: string;
  division: number;
  init_rank: number;
  players: number;
  games_played: number;
  wins: number;
  losses: number;
  draws: number;
  spirit_avg: number;
  spirit_rank: number;
  current_rank: number;
  full_logo?: string | null;
  small_logo?: string | null;
}

interface PlayerStat {
  id: number;
  full_name: string;
  common_name?: string | null;
  goals: number;
  assists: number;
  blocks: number;
  turnovers: number;
  is_captain: boolean;
  is_spirit_captain: boolean;
  is_manager: boolean;
  is_coach: boolean;
}

interface TeamMatch {
  id: number;
  t1_id: number;
  t2_id: number;
  t1_name: string;
  t2_name: string;
  t1_abbreviation?: string | null;
  t2_abbreviation?: string | null;
  t1_score: number;
  t2_score: number;
  t1_spirit: number | null;
  t2_spirit: number | null;
  field_name: string;
  time: string;
  possession: number | null;
  stream_url: string | null;
  match_type: number;
}

interface TeamSeedTimelinePoint {
  label: string;
  seed: number;
}

type TabType = 'matches' | 'players' | 'timeline';
type PlayerSortField = 'total' | 'goals' | 'assists' | 'blocks' | 'turnovers';
interface TeamsPreferences {
  activeTab: TabType;
}

const TEAMS_PREFS_KEY = 'sakkath:teams:preferences';

function getTotalStat(player: Pick<PlayerStat, 'goals' | 'assists' | 'blocks' | 'turnovers'>) {
  return player.goals + player.assists + player.blocks - player.turnovers;
}

function getPlayerRoleLabel(player: Pick<PlayerStat, 'is_captain' | 'is_spirit_captain' | 'is_manager' | 'is_coach'>) {
  if (player.is_captain) return 'Captain';
  if (player.is_spirit_captain) return 'Spirit Captain';
  if (player.is_manager) return 'Manager';
  if (player.is_coach) return 'Coach';
  return 'Player';
}

function getInitialActiveTab(): TabType {
  if (typeof window === 'undefined') {
    return 'matches';
  }

  const savedPreferences = window.localStorage.getItem(TEAMS_PREFS_KEY);
  if (!savedPreferences) {
    return 'matches';
  }

  try {
    const parsed: TeamsPreferences = JSON.parse(savedPreferences);
    if (parsed.activeTab === 'matches' || parsed.activeTab === 'players' || parsed.activeTab === 'timeline') {
      return parsed.activeTab;
    }
  } catch {
  }

  return 'matches';
}

function TeamContent() {
  const searchParams = useSearchParams();
  const router = useRouter();
  const [team, setTeam] = useState<Team | null>(null);
  const [playerStats, setPlayerStats] = useState<PlayerStat[]>([]);
  const [matches, setMatches] = useState<TeamMatch[]>([]);
  const [seedTimeline, setSeedTimeline] = useState<TeamSeedTimelinePoint[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<TabType>(getInitialActiveTab);
  const [playerSortField, setPlayerSortField] = useState<PlayerSortField>('total');
  const [playerSortDir, setPlayerSortDir] = useState<'asc' | 'desc'>('desc');
  const [showingCommonNames, setShowingCommonNames] = useState<Record<number, boolean>>({});
  const teamId = searchParams.get('team_id') || '1';

  useEffect(() => {
    const preferences: TeamsPreferences = { activeTab };
    localStorage.setItem(TEAMS_PREFS_KEY, JSON.stringify(preferences));
  }, [activeTab]);

  useEffect(() => {
    Promise.all([
      fetch(apiUrl(`/v1/teams/${teamId}`)).then(r => r.json()),
      fetch(apiUrl(`/v1/teams/${teamId}/players`)).then(r => r.json()),
      fetch(apiUrl(`/v1/teams/${teamId}/matches`)).then(r => r.json()),
      fetch(apiUrl(`/v1/teams/${teamId}/seed-timeline`)).then(r => r.ok ? r.json() : []),
    ]).then(([teamData, playersData, matchesData, seedTimelineData]) => {
      setTeam(teamData);
      setPlayerStats(playersData);
      setMatches(matchesData);
      setSeedTimeline(seedTimelineData);
      setShowingCommonNames({});
      setLoading(false);
    }).catch(() => setLoading(false));
  }, [teamId]);

  if (loading || !team) {
    return (
      <div className="py-4 px-4 md:px-0 min-h-screen">
        <div className="max-w-7xl mx-auto">
          <Text variant="primary">{loading ? 'Loading...' : 'Team not found'}</Text>
        </div>
      </div>
    );
  }

  const getMatchStatus = (match: TeamMatch) => {
    if (match.possession === null) return 'upcoming';
    if (match.possession >= 3) return 'done';
    return 'live';
  };

  const formatTime = (time: string) => {
    if (!time) return '';
    return formatIndiaShortDateTime(time);
  };

  const getCardTeamName = (name: string, abbreviation?: string | null) => {
    if (name.length > 16) {
      return getTeamAbbreviation(name, abbreviation ?? undefined, 16);
    }

    return name;
  };

  const getRoundLabel = (type: number) => {
    if (type === 1001) return 'Playoff 1';
    if (type === 1002) return 'Playoff 2';
    return `Round ${type}`;
  };

  const togglePlayerName = (playerId: number) => {
    setShowingCommonNames((current) => ({
      ...current,
      [playerId]: !current[playerId],
    }));
  };

  const renderPlayerName = (player: PlayerStat) => {
    const commonName = player.common_name?.trim() || "Player";
    const hasAlternateName = Boolean(commonName && commonName !== player.full_name);

    if (!hasAlternateName) {
      return <Text variant="primary" className="font-medium truncate max-w-[150px] pointer-events-none">{abbreviatePlayerName(player.full_name, 20)}</Text>;
    }

    const showingCommonName = Boolean(showingCommonNames[player.id]);
    const displayName = abbreviatePlayerName(player.full_name, 20);
    const displayCommonName = abbreviatePlayerName(commonName, 20);

    return (
      <div className="relative inline-block max-w-[150px] w-full pointer-events-none">
        <span className="relative block min-h-[1.5rem] w-full overflow-hidden">
          <span
            className={`block truncate font-medium text-gray-900 transition-all duration-300 dark:text-white pointer-events-none ${
              showingCommonName ? '-translate-y-full scale-95 opacity-0' : 'translate-y-0 scale-100 opacity-100'
            }`}
            title={player.full_name}
          >
            {displayName}
          </span>
          <span
            className={`absolute inset-0 block truncate font-medium text-blue-700 transition-all duration-300 dark:text-cyan-300 pointer-events-none ${
              showingCommonName ? 'translate-y-0 scale-100 opacity-100' : 'translate-y-full scale-95 opacity-0'
            }`}
            title={commonName}
          >
            {displayCommonName}
          </span>
        </span>
      </div>
    );
  };

  return (
    <div className="py-4 px-4 md:px-0 min-h-screen overflow-x-hidden">
      <div className="max-w-7xl mx-auto">
        {/* Team Info Panel */}
        <div className="rounded-sm p-6 mb-5 bg-white dark:bg-slate-900">
          <div className="flex flex-col md:flex-row items-center gap-6">
            <div className="flex-shrink-0 w-[120px] h-[120px] relative bg-gray-100 dark:bg-slate-800 rounded-full flex items-center justify-center overflow-hidden">
              {team.full_logo ? (
                <img src={team.full_logo} alt={team.name} className="w-full h-full object-cover" />
              ) : (
                <Text variant="secondary" className="text-4xl font-bold">{team.name.charAt(0).toUpperCase()}</Text>
              )}
            </div>
            <div className="flex-1">
              <Text as="h1" variant="primary" className="text-2xl font-semibold mb-2">
                {team.name} ({team.division === 0 ? 'Open' : 'Women'})
              </Text>
              <div className="flex flex-wrap gap-4 text-sm mb-3">
                <Text variant="secondary" className="flex items-center gap-1">
                  <MapPin className="w-4 h-4" />
                  {team.location}, India
                </Text>
              </div>
            </div>
          </div>
          
          {/* Stats Row */}
          <div className="flex flex-wrap justify-center gap-3 mt-6 pt-6 border-t border-gray-200 dark:border-slate-700">
            <div className="hidden sm:flex flex-wrap justify-center gap-3 w-full">
              {[
                { icon: Users, value: team.players.toString(), label: 'Players' },
                { icon: Trophy, value: team.games_played.toString(), label: 'Games Played' },
                { icon: Target, value: team.wins.toString(), label: 'Won', color: 'text-green-500' },
                { icon: Target, value: team.losses.toString(), label: 'Lost', color: 'text-red-500' },
                { icon: Sparkle, value: team.spirit_avg.toFixed(1), label: 'Spirit Avg' },
                { icon: AlignStartVertical, value: team.spirit_rank.toString(), label: 'Spirit Rank' },
                { icon: Star, value: team.init_rank.toString(), label: 'Init Rank' },
                { icon: team.current_rank < team.init_rank ? TrendingUp : TrendingDown, value: team.current_rank.toString(), label: 'Curr Rank', color: team.current_rank < team.init_rank ? 'text-green-500' : team.current_rank > team.init_rank ? 'text-red-500' : '' },
              ].map((stat, i) => (
                <div 
                  key={i}
                  className="flex items-center gap-2 px-4 py-2 rounded bg-gray-100 dark:bg-slate-800"
                >
                  <stat.icon className={`w-4 h-4 ${stat.color || 'text-gray-500 dark:text-gray-400'}`} />
                  <Text variant="primary" className={`font-bold ${stat.color || ''}`}>{stat.value}</Text>
                  <Text variant="secondary" className="text-sm">{stat.label}</Text>
                  {stat.label === 'Curr Rank' && team.current_rank !== team.init_rank && (
                    team.current_rank < team.init_rank ? (
                      <ArrowUp className="w-4 h-4 text-green-500" />
                    ) : (
                      <ArrowDown className="w-4 h-4 text-red-500" />
                    )
                  )}
                </div>
              ))}
            </div>

            {/* Mobile Stats */}
            <div className="flex sm:hidden flex-col gap-3 w-full">
              <div className="flex gap-3">
                {[
                  { icon: Users, value: team.players.toString(), label: 'Players' },
                  { icon: Trophy, value: team.games_played.toString(), label: 'Games' },
                ].map((stat, i) => (
                  <div key={i} className="flex-1 flex items-center gap-2 px-3 py-2 rounded bg-gray-100 dark:bg-slate-800">
                    <stat.icon className="w-4 h-4 text-gray-500 dark:text-gray-400" />
                    <Text variant="primary" className="font-bold text-sm">{stat.value}</Text>
                    <Text variant="secondary" className="text-xs">{stat.label}</Text>
                  </div>
                ))}
              </div>
              <div className="flex gap-3">
                {[
                  { icon: Target, value: team.wins.toString(), label: 'Won', color: 'text-green-500' },
                  { icon: Target, value: team.losses.toString(), label: 'Lost', color: 'text-red-500' },
                ].map((stat, i) => (
                  <div key={i} className="flex-1 flex items-center gap-2 px-3 py-2 rounded bg-gray-100 dark:bg-slate-800">
                    <stat.icon className={`w-4 h-4 ${stat.color}`} />
                    <Text variant="primary" className={`font-bold text-sm ${stat.color}`}>{stat.value}</Text>
                    <Text variant="secondary" className="text-xs">{stat.label}</Text>
                  </div>
                ))}
              </div>
              <div className="flex gap-3">
                {[
                  { icon: Sparkle, value: team.spirit_avg.toFixed(1), label: 'Spirit Avg' },
                  { icon: AlignStartVertical, value: team.spirit_rank.toString(), label: 'Spirit Rank' },
                ].map((stat, i) => (
                  <div key={i} className="flex-1 flex items-center gap-2 px-3 py-2 rounded bg-gray-100 dark:bg-slate-800">
                    <stat.icon className="w-4 h-4 text-gray-500 dark:text-gray-400" />
                    <Text variant="primary" className="font-bold text-sm">{stat.value}</Text>
                    <Text variant="secondary" className="text-xs">{stat.label}</Text>
                  </div>
                ))}
              </div>
              <div className="flex gap-3">
                {[
                  { icon: Star, value: team.init_rank.toString(), label: 'Init Rank' },
                  { icon: team.current_rank < team.init_rank ? TrendingUp : TrendingDown, value: team.current_rank.toString(), label: 'Curr Rank', color: team.current_rank < team.init_rank ? 'text-green-500' : team.current_rank > team.init_rank ? 'text-red-500' : '' },
                ].map((stat, i) => (
                  <div key={i} className="flex-1 flex items-center gap-2 px-3 py-2 rounded bg-gray-100 dark:bg-slate-800">
                    <stat.icon className={`w-4 h-4 ${stat.color || 'text-gray-500 dark:text-gray-400'}`} />
                    <Text variant="primary" className={`font-bold text-sm ${stat.color || ''}`}>{stat.value}</Text>
                    <Text variant="secondary" className="text-xs">{stat.label}</Text>
                    {stat.label === 'Curr Rank' && team.current_rank !== team.init_rank && (
                      team.current_rank < team.init_rank ? (
                        <ArrowUp className="w-4 h-4 text-green-500" />
                      ) : (
                        <ArrowDown className="w-4 h-4 text-red-500" />
                      )
                    )}
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>

        {/* Tabs Panel */}
        <div className="rounded-sm p-6 bg-white dark:bg-slate-900">
          <div className="flex gap-2 mb-4">
            {(['matches', 'players', 'timeline'] as TabType[]).map((tab) => (
              <button
                key={tab}
                onClick={() => setActiveTab(tab)}
                className={`px-4 py-2 text-sm font-medium rounded transition-colors ${
                  activeTab === tab
                    ? 'bg-blue-900 text-white'
                    : 'bg-gray-200 dark:bg-slate-700 text-gray-600 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-slate-600'
                }`}
              >
                {tab.charAt(0).toUpperCase() + tab.slice(1)}
              </button>
            ))}
          </div>

          {/* Matches Tab */}
          {activeTab === 'matches' && (
            <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
              {matches.map((match) => {
                const status = getMatchStatus(match);
                const isT1 = match.t1_id === team.id;
                const ourScore = isT1 ? match.t1_score : match.t2_score;
                const theirScore = isT1 ? match.t2_score : match.t1_score;
                const ourSpirit = isT1 ? match.t1_spirit : match.t2_spirit;
                const theirSpirit = isT1 ? match.t2_spirit : match.t1_spirit;
                const opponent = isT1 ? match.t2_name : match.t1_name;
                const ourDisplayName = getCardTeamName(team.name, team.abbreviation);
                const opponentDisplayName = isT1
                  ? getCardTeamName(match.t2_name, match.t2_abbreviation)
                  : getCardTeamName(match.t1_name, match.t1_abbreviation);
                
                return (
                  <div 
                    key={match.id} 
                    className="p-4 rounded bg-gray-50 dark:bg-slate-800 border border-gray-200 dark:border-slate-700 cursor-pointer hover:border-blue-500 transition-colors"
                    onClick={() => router.push(`/matches?match_id=${match.id}`)}
                  >
                    <div className="flex items-center justify-between mb-3">
                      <Text as="div" variant="secondary" className="text-xs">{`${getRoundLabel(match.match_type)} ${formatTime(match.time)}`}</Text>
                      <div className="flex items-center gap-2">
                        {status === 'live' && (
                          <>
                            <Circle className="w-3 h-3 fill-red-500 text-red-500" />
                            <Text variant="primary" className="text-xs font-medium text-red-500">Live</Text>
                          </>
                        )}
                        {status === 'done' && (
                          <>
                            <Circle className="w-3 h-3 fill-gray-400 text-gray-400" />
                            <Text variant="secondary" className="text-xs">Ended</Text>
                          </>
                        )}
                        {status === 'upcoming' && (
                          <>
                            <Circle className="w-3 h-3 fill-yellow-400 text-yellow-400" />
                            <Text variant="primary" className="text-xs font-medium text-yellow-500">Upcoming</Text>
                          </>
                        )}
                        {match.stream_url && (
                          <a href={match.stream_url} target="_blank" rel="noopener noreferrer" className="text-blue-500 hover:text-blue-400">
                            <Play className="w-4 h-4" />
                          </a>
                        )}
                      </div>
                    </div>
                    
                    <div className="space-y-2">
                      <div className="flex items-center justify-between gap-2">
                        <Text variant="primary" className="font-medium truncate min-w-0 flex-1">{ourDisplayName}</Text>
                        <Text variant="primary" className="font-bold text-lg shrink-0">{status === 'upcoming' ? '-' : ourScore}</Text>
                      </div>
                      <div className="flex items-center justify-between gap-2">
                        <Text variant="secondary" className="truncate min-w-0 flex-1">{opponentDisplayName}</Text>
                        <Text variant="secondary" className="font-bold text-lg shrink-0">{status === 'upcoming' ? '-' : theirScore}</Text>
                      </div>
                    </div>
                    
                    <div className="mt-3 pt-3 border-t border-gray-200 dark:border-slate-700 flex items-center justify-between">
                      {ourSpirit !== null && theirSpirit !== null ? (
                        <div className="flex gap-4">
                          <div className="text-center">
                            <Text variant="secondary" className="text-xs">Spirit</Text>
                            <Text variant="primary" className="text-sm font-extrabold">{ourSpirit}</Text>
                          </div>
                          <div className="text-center">
                            <Text variant="secondary" className="text-xs">Spirit</Text>
                            <Text variant="secondary" className="text-sm">{theirSpirit}</Text>
                          </div>
                        </div>
                      ) : (
                        <div />
                      )}
                      <Text variant="secondary" className="text-xs">{match.field_name}</Text>
                    </div>
                  </div>
                );
              })}
              {matches.length === 0 && <Text variant="secondary">No matches scheduled</Text>}
            </div>
          )}

          {/* Players Tab */}
          {activeTab === 'players' && (
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
                      const active = playerSortField === field;
                      return (
                        <th
                          key={field}
                          className={`w-12 py-3 px-1.5 font-medium cursor-pointer hover:text-gray-700 dark:hover:text-gray-200 ${active ? 'text-gray-900 dark:text-white' : 'text-gray-500 dark:text-gray-400'}`}
                          onClick={() => {
                            if (active) {
                              setPlayerSortDir((current) => current === 'asc' ? 'desc' : 'asc');
                            } else {
                              setPlayerSortField(field);
                              setPlayerSortDir('desc');
                            }
                          }}
                        >
                          <div className="flex items-center justify-center gap-1 whitespace-nowrap">
                            <span>{label}</span>
                            {active && (playerSortDir === 'asc' ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />)}
                          </div>
                        </th>
                      );
                    })}
                  </tr>
                </thead>
                <tbody>
                  {[...playerStats]
                    .sort((a, b) => {
                      const diff = (playerSortField === 'total' ? getTotalStat(a) : a[playerSortField]) - (playerSortField === 'total' ? getTotalStat(b) : b[playerSortField]);
                      return playerSortDir === 'asc' ? diff : -diff;
                    })
                    .map((player) => (
                    <tr 
                      key={player.id} 
                      onClick={() => togglePlayerName(player.id)}
                      className="hover:opacity-80 border-b border-gray-200 dark:border-slate-700 cursor-pointer"
                    >
                      <td className="w-[168px] py-3 px-1.5">
                        <div className="flex flex-col justify-center min-w-0">
                          {renderPlayerName(player)}
                          <Text variant="secondary" className="text-[10px] leading-tight truncate">
                            {getPlayerRoleLabel(player)}
                          </Text>
                        </div>
                      </td>
                      <td className="w-12 py-3 px-1.5 text-center"><Text variant="primary">{getTotalStat(player)}</Text></td>
                      <td className="w-12 py-3 px-1.5 text-center"><Text variant="primary">{player.goals}</Text></td>
                      <td className="w-12 py-3 px-1.5 text-center"><Text variant="primary">{player.assists}</Text></td>
                      <td className="w-12 py-3 px-1.5 text-center"><Text variant="primary">{player.blocks}</Text></td>
                      <td className="w-12 py-3 px-1.5 text-center"><Text variant="primary">{player.turnovers}</Text></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {/* Timeline Tab */}
          {activeTab === 'timeline' && (
            <div>
              {(() => {
                if (seedTimeline.length <= 1) {
                  return <Text variant="secondary">No completed matches yet</Text>;
                }

                const seedData = seedTimeline.map((point) => ({
                  stage: point.label,
                  seed: point.seed,
                  tooltip: `${point.label}: Seed ${point.seed}`,
                }));

                return (
                  <div className="mb-6">
                    <div className="w-full overflow-x-auto overflow-y-hidden">
                      <div style={{ width: `${Math.max(720, seedData.length * 56)}px`, height: '500px' }}>
                          <LineChart data={seedData} width={Math.max(720, seedData.length * 56)} height={500} margin={{ top: 30, right: 30, left: 10, bottom: 80 }}>
                            <CartesianGrid strokeDasharray="3 3" stroke="#374151" opacity={0.3} />
                            <XAxis 
                              dataKey="stage" 
                              stroke="#9ca3af" 
                              fontSize={12}
                              tick={{ fill: '#9ca3af' }}
                              label={{ value: 'Stage', position: 'insideBottom', offset: -15, fill: '#9ca3af', fontWeight: 500 }}
                            />
                            <YAxis 
                              stroke="#9ca3af" 
                              fontSize={12}
                              tick={{ fill: '#9ca3af' }}
                              domain={[1, 'dataMax + 1']}
                              reversed
                              allowDecimals={false}
                              label={{ value: 'Seed', angle: -90, position: 'insideLeft', offset: 5, fill: '#9ca3af', fontWeight: 500 }}
                            />
                            <Tooltip 
                              contentStyle={{ 
                                backgroundColor: '#253041ff', 
                                border: 'none', 
                                borderRadius: '8px',
                                color: '#fff',
                                fontSize: '12px',
                                padding: '8px'
                              }}
                              labelFormatter={(label, payload) => {
                                if (payload && payload.length > 0) {
                                  return payload[0].payload.tooltip;
                                }
                                return label;
                              }}
                            />
                            <Line 
                              type="linear" 
                              dataKey="seed" 
                              stroke="#8fa6d8ff" 
                              strokeWidth={3}
                              dot={{ fill: '#2563eb', strokeWidth: 2, r: 6 }}
                              activeDot={{ r: 8, fill: '#2563eb' }}
                              animationDuration={1500}
                              animationEasing="ease-out"
                            />
                          </LineChart>
                      </div>
                    </div>
                  </div>
                );
              })()}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default function Teams() {
  return (
    <Suspense fallback={<div className="py-8 px-4 md:px-0"><div className="max-w-7xl mx-auto"><Text variant="primary">Loading...</Text></div></div>}>
      <TeamContent />
    </Suspense>
  );
}
