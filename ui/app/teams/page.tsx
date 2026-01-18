'use client';

import { useEffect, useState, Suspense } from "react";
import { useSearchParams } from "next/navigation";
import { MapPin, Users, Trophy, Target, ArrowUp, ArrowDown, Star, TrendingUp, TrendingDown, Sparkle, AlignStartVertical } from "lucide-react";
import { Text } from "../components/Text";

interface Team {
  id: number;
  name: string;
  location: string;
  division: number;
  init_rank: number;
  players: number;
  games_played: number;
  wins: number;
  losses: number;
  spirit_avg: number;
  spirit_rank: number;
  current_rank: number;
  full_logo?: string | null;
  small_logo?: string | null;
}

interface PlayerStat {
  id: number;
  name: string;
  goals: number;
  assists: number;
  blocks: number;
  turnovers: number;
  is_captain: boolean;
  is_spirit_captain: boolean;
}

const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:9000';

function TeamContent() {
  const searchParams = useSearchParams();
  const [mounted, setMounted] = useState(false);
  const [team, setTeam] = useState<Team | null>(null);
  const [playerStats, setPlayerStats] = useState<PlayerStat[]>([]);
  const [loading, setLoading] = useState(true);
  const teamId = searchParams.get('team_id') || '1';

  useEffect(() => {
    setMounted(true);
    Promise.all([
      fetch(`${API_URL}/v1/teams/${teamId}`).then(r => r.json()),
      fetch(`${API_URL}/v1/teams/${teamId}/players`).then(r => r.json()),
    ]).then(([teamData, playersData]) => {
      setTeam(teamData);
      setPlayerStats(playersData);
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

  const divisionName = team.division === 0 ? 'Open Division' : 'Women Division';

  return (
    <div className="py-4 px-4 md:px-0 min-h-screen">
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
                { icon: team.init_rank <= team.current_rank ? TrendingUp : TrendingDown, value: team.current_rank.toString(), label: 'Curr Rank', color: team.current_rank < team.init_rank ? 'text-green-500' : team.current_rank > team.init_rank ? 'text-red-500' : '' },
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
                  { icon: team.init_rank <= team.current_rank ? TrendingUp : TrendingDown, value: team.current_rank.toString(), label: 'Curr Rank', color: team.current_rank < team.init_rank ? 'text-green-500' : team.current_rank > team.init_rank ? 'text-red-500' : '' },
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

        {/* Player Stats Panel */}
        <div className="rounded-sm p-6 bg-white dark:bg-slate-900">
          <Text as="h2" variant="primary" className="text-xl mb-4">
            Player Statistics
          </Text>
          
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-gray-200 dark:border-slate-700">
                  <th className="py-3 px-2 text-left font-medium text-gray-500 dark:text-gray-400">Player</th>
                  <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">Goals</th>
                  <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">Assists</th>
                  <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">Blocks</th>
                  <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">Turnovers</th>
                </tr>
              </thead>
              <tbody>
                {playerStats.map((player) => (
                  <tr key={player.id} className="hover:opacity-80 border-b border-gray-200 dark:border-slate-700">
                    <td className="py-3 px-2">
                      <div className="flex items-center gap-2">
                        <Text variant="primary" className="font-medium">{player.name}</Text>
                        {player.is_captain && (
                          <span className="w-6 h-6 rounded flex items-center justify-center bg-yellow-500 text-white text-xs font-bold">
                            C
                          </span>
                        )}
                        {player.is_spirit_captain && (
                          <span className="w-6 h-6 rounded flex items-center justify-center bg-purple-500 text-white text-xs font-bold">
                            SC
                          </span>
                        )}
                      </div>
                    </td>
                    <td className="py-3 px-2 text-center">
                      <Text variant="primary">{player.goals}</Text>
                    </td>
                    <td className="py-3 px-2 text-center">
                      <Text variant="primary">{player.assists}</Text>
                    </td>
                    <td className="py-3 px-2 text-center">
                      <Text variant="primary">{player.blocks}</Text>
                    </td>
                    <td className="py-3 px-2 text-center">
                      <Text variant="primary">{player.turnovers}</Text>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
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
