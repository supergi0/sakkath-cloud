'use client';

import { useEffect, useState } from "react";
import { useTheme } from "next-themes";
import Image from "next/image";
import Link from "next/link";
import { Calendar, MapPin, Clock, Users, Trophy, Target, LayoutGrid } from "lucide-react";
import { Text } from "./components/Text";

interface TeamStanding {
  id: number;
  name: string;
  location: string;
  init_rank: number;
  wins: number;
  losses: number;
  points_for: number;
  points_against: number;
  spirit_avg: number;
  small_logo?: string | null;
}

interface Stats {
  teams: number;
  players: number;
  points: number;
  games: number;
  fields: number;
}

const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:9000';

type SortBy = 'game' | 'initial' | 'spirit';

export default function Home() {
  const { theme } = useTheme();
  const [mounted, setMounted] = useState(false);
  const [division, setDivision] = useState<'open' | 'women'>('open');
  const [sortBy, setSortBy] = useState<SortBy>('game');
  const [openStandings, setOpenStandings] = useState<TeamStanding[]>([]);
  const [womenStandings, setWomenStandings] = useState<TeamStanding[]>([]);
  const [stats, setStats] = useState<Stats>({ teams: 0, players: 0, points: 0, games: 0, fields: 0 });
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setMounted(true);
    Promise.all([
      fetch(`${API_URL}/v1/standings?division=0`).then(r => r.json()),
      fetch(`${API_URL}/v1/standings?division=1`).then(r => r.json()),
      fetch(`${API_URL}/v1/stats`).then(r => r.json()),
    ]).then(([open, women, statsData]) => {
      setOpenStandings(open);
      setWomenStandings(women);
      setStats(statsData);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  const isDark = mounted && theme === 'dark';
  
  const sortStandings = (standings: TeamStanding[]) => {
    return [...standings].sort((a, b) => {
      switch (sortBy) {
        case 'initial':
          return a.init_rank - b.init_rank;
        case 'spirit':
          return b.spirit_avg - a.spirit_avg;
        case 'game':
        default:
          const aWinPct = a.wins / (a.wins + a.losses || 1);
          const bWinPct = b.wins / (b.wins + b.losses || 1);
          if (bWinPct !== aWinPct) return bWinPct - aWinPct;
          return (b.points_for - b.points_against) - (a.points_for - a.points_against);
      }
    });
  };

  const currentStandings = sortStandings(division === 'open' ? openStandings : womenStandings);

  if (!mounted) return null;

  return (
    <div className="py-4 px-4 md:px-0 min-h-screen">
      <div className="max-w-7xl mx-auto">
        {/* Overview Panel */}
        <div className="rounded-sm p-6 mb-5 bg-white dark:bg-slate-900">
          <div className="flex flex-col md:flex-row items-center gap-6">
            {/* Mobile: Logo and Title Sponsor in same row */}
            <div className="flex md:hidden items-center gap-4 w-full justify-between">
              <div className="flex-shrink-0 w-[140px] h-[55px] relative">
                {mounted && (
                  <Image
                    src={isDark ? '/sakkath_dark.png' : '/sakkath_light.png'}
                    alt="Sakkath Ultimate Open"
                    fill
                    className="object-contain"
                    priority
                  />
                )}
              </div>
              <div className="flex-shrink-0 w-[110px] h-[55px] relative bg-gray-100 dark:bg-slate-800 rounded flex items-center justify-center">
                <Text variant="secondary" className="text-xs font-medium">
                  Title Sponsor
                </Text>
              </div>
            </div>

            {/* Desktop: Logo on left */}
            <div className="hidden md:block flex-shrink-0 w-[180px] h-[70px] relative">
              {mounted && (
                <Image
                  src={isDark ? '/sakkath_dark.png' : '/sakkath_light.png'}
                  alt="Sakkath Ultimate Open"
                  fill
                  className="object-contain"
                  priority
                />
              )}
            </div>

            <div className="flex-1">
              <Text as="h1" variant="primary" className="text-2xl font-semibold mb-2">
                Sakkath Ultimate Open 2026
              </Text>
              <div className="flex flex-wrap gap-4 text-sm">
                <Text variant="secondary" className="flex items-center gap-1">
                  <Calendar className="w-4 h-4" />
                  January 15 - January 18, 2026
                </Text>
                <Text variant="secondary" className="flex items-center gap-1">
                  <MapPin className="w-4 h-4" />
                  Bangalore, India
                </Text>
                <Text variant="secondary" className="flex items-center gap-1">
                  <Clock className="w-4 h-4" />
                  UTC+5:30
                </Text>
              </div>
            </div>

            {/* Desktop: Title Sponsor on right */}
            <div className="hidden md:block flex-shrink-0 w-[140px] h-[70px] relative bg-gray-100 dark:bg-slate-800 rounded flex items-center justify-center">
              <Text variant="secondary" className="text-sm font-medium">
                Title Sponsor
              </Text>
            </div>
          </div>
          
          {/* Stats Row */}
          <div className="flex flex-wrap justify-center gap-3 mt-6 pt-6 border-t border-gray-200 dark:border-slate-700">
            <div className="hidden sm:flex flex-wrap justify-center gap-3 w-full">
              {[
                { icon: Users, value: stats.teams.toString(), label: 'Teams' },
                { icon: Users, value: stats.players.toString(), label: 'Players' },
                { icon: Target, value: stats.points.toString(), label: 'Points' },
                { icon: Trophy, value: stats.games.toString(), label: 'Games' },
                { icon: LayoutGrid, value: stats.fields.toString(), label: 'Fields' },
                { icon: Calendar, value: '3', label: 'Days' },
              ].map((stat, i) => (
                <div 
                  key={i}
                  className="flex items-center gap-2 px-4 py-2 rounded bg-gray-100 dark:bg-slate-800"
                >
                  <stat.icon className="w-4 h-4 text-gray-500 dark:text-gray-400" />
                  <Text variant="primary" className="font-bold">{stat.value}</Text>
                  <Text variant="secondary" className="text-sm">{stat.label}</Text>
                </div>
              ))}
            </div>

            {/* Mobile Stats */}
            <div className="flex sm:hidden flex-col gap-3 w-full">
              <div className="flex gap-3">
                {[
                  { icon: Users, value: stats.teams.toString(), label: 'Teams' },
                  { icon: Users, value: stats.players.toString(), label: 'Players' },
                ].map((stat, i) => (
                  <div 
                    key={i}
                    className="flex-1 flex items-center gap-2 px-3 py-2 rounded bg-gray-100 dark:bg-slate-800"
                  >
                    <stat.icon className="w-4 h-4 text-gray-500 dark:text-gray-400" />
                    <Text variant="primary" className="font-bold text-sm">{stat.value}</Text>
                    <Text variant="secondary" className="text-xs">{stat.label}</Text>
                  </div>
                ))}
              </div>
              <div className="flex gap-3">
                {[
                  { icon: Target, value: stats.points.toString(), label: 'Points' },
                  { icon: Trophy, value: stats.games.toString(), label: 'Games' },
                ].map((stat, i) => (
                  <div 
                    key={i}
                    className="flex-1 flex items-center gap-2 px-3 py-2 rounded bg-gray-100 dark:bg-slate-800"
                  >
                    <stat.icon className="w-4 h-4 text-gray-500 dark:text-gray-400" />
                    <Text variant="primary" className="font-bold text-sm">{stat.value}</Text>
                    <Text variant="secondary" className="text-xs">{stat.label}</Text>
                  </div>
                ))}
              </div>
              <div className="flex gap-3">
                {[
                  { icon: LayoutGrid, value: stats.fields.toString(), label: 'Fields' },
                  { icon: Calendar, value: '3', label: 'Days' },
                ].map((stat, i) => (
                  <div 
                    key={i}
                    className="flex-1 flex items-center gap-2 px-3 py-2 rounded bg-gray-100 dark:bg-slate-800"
                  >
                    <stat.icon className="w-4 h-4 text-gray-500 dark:text-gray-400" />
                    <Text variant="primary" className="font-bold text-sm">{stat.value}</Text>
                    <Text variant="secondary" className="text-xs">{stat.label}</Text>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>

        {/* Standings Panel */}
        <div className="rounded-sm p-6 bg-white dark:bg-slate-900">
          {/* Desktop Layout */}
          <div className="hidden sm:flex items-center justify-between gap-3 mb-4">
            <div className="flex items-center gap-4">
              <Text as="h2" variant="primary" className="text-xl">
                Standings
              </Text>
              <div className="flex gap-1">
                {(['game', 'initial', 'spirit'] as SortBy[]).map((s) => (
                  <button
                    key={s}
                    onClick={() => setSortBy(s)}
                    className={`px-3 py-2 text-sm rounded transition-colors ${
                      sortBy === s
                        ? 'bg-cyan-900 text-white'
                        : 'bg-gray-200 dark:bg-slate-700 text-gray-600 dark:text-gray-300'
                    }`}
                  >
                    {s.charAt(0).toUpperCase() + s.slice(1)}
                  </button>
                ))}
              </div>
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => setDivision('open')}
                className={`px-4 py-2 rounded text-sm font-medium transition-colors ${
                  division === 'open'
                    ? 'bg-cyan-900 text-white'
                    : 'bg-gray-200 dark:bg-slate-800 text-gray-700 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-slate-700'
                }`}
              >
                Open Division
              </button>
              <button
                onClick={() => setDivision('women')}
                className={`px-4 py-2 rounded text-sm font-medium transition-colors ${
                  division === 'women'
                    ? 'bg-cyan-900 text-white'
                    : 'bg-gray-200 dark:bg-slate-800 text-gray-700 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-slate-700'
                }`}
              >
                Women Division
              </button>
            </div>
          </div>

          {/* Mobile Layout */}
          <div className="flex sm:hidden flex-col gap-3 mb-4">
            <Text as="h2" variant="primary" className="text-xl">
              Standings
            </Text>
            <div className="flex gap-2">
              {(['game', 'initial', 'spirit'] as SortBy[]).map((s) => (
                <button
                  key={s}
                  onClick={() => setSortBy(s)}
                  className={`flex-1 px-3 py-2 text-sm rounded transition-colors ${
                    sortBy === s
                      ? 'bg-cyan-900 text-white'
                      : 'bg-gray-200 dark:bg-slate-700 text-gray-600 dark:text-gray-300'
                  }`}
                >
                  {s === 'game' ? 'Game' : s === 'initial' ? 'Initial' : 'Spirit'}
                </button>
              ))}
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => setDivision('open')}
                className={`flex-1 px-3 py-2.5 rounded text-sm font-medium transition-colors ${
                  division === 'open'
                    ? 'bg-cyan-900 text-white'
                    : 'bg-gray-200 dark:bg-slate-800 text-gray-700 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-slate-700'
                }`}
              >
                Open
              </button>
              <button
                onClick={() => setDivision('women')}
                className={`flex-1 px-3 py-2.5 rounded text-sm font-medium transition-colors ${
                  division === 'women'
                    ? 'bg-cyan-900 text-white'
                    : 'bg-gray-200 dark:bg-slate-800 text-gray-700 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-slate-700'
                }`}
              >
                Women
              </button>
            </div>
          </div>
          
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-gray-200 dark:border-slate-700">
                  <th className="py-3 px-2 text-left font-medium text-gray-500 dark:text-gray-400">#</th>
                  <th className="py-3 px-2 text-left font-medium text-gray-500 dark:text-gray-400">Team</th>
                  <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">P</th>
                  <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">W</th>
                  <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">L</th>
                  <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">PF</th>
                  <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">PA</th>
                  <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">Diff</th>
                  <th className="py-3 px-2 text-center font-medium text-gray-500 dark:text-gray-400">Spirit</th>
                </tr>
              </thead>
              <tbody>
                {currentStandings.map((team, idx) => {
                  const diff = team.points_for - team.points_against;
                  const initial = team.name.charAt(0).toUpperCase();
                  return (
                    <tr 
                      key={team.id} 
                      className="hover:opacity-80 border-b border-gray-200 dark:border-slate-700"
                    >
                      <td className="py-3 px-2">
                        <Text variant="primary" className="font-medium">{idx + 1}</Text>
                      </td>
                      <td className="py-3 px-2">
                        <Link href={`/teams?team_id=${team.id}`} className="flex items-center gap-2 hover:underline">
                          <div className="w-7 h-7 sm:w-6 sm:h-6 rounded-full bg-cyan-900 text-white flex items-center justify-center text-xs font-bold flex-shrink-0 overflow-hidden">
                            {team.small_logo ? (
                              <img src={team.small_logo} alt={team.name} className="w-full h-full object-cover" />
                            ) : (
                              initial
                            )}
                          </div>
                          <Text variant="primary" className="font-medium">{team.name}</Text>
                        </Link>
                      </td>
                      <td className="py-3 px-2 text-center">
                        <Text variant="secondary">{team.wins + team.losses}</Text>
                      </td>
                      <td className="py-3 px-2 text-center text-green-500">{team.wins}</td>
                      <td className="py-3 px-2 text-center text-red-500">{team.losses}</td>
                      <td className="py-3 px-2 text-center">
                        <Text variant="primary">{team.points_for}</Text>
                      </td>
                      <td className="py-3 px-2 text-center">
                        <Text variant="primary">{team.points_against}</Text>
                      </td>
                      <td className={`py-3 px-2 text-center font-medium ${diff > 0 ? 'text-green-500' : diff < 0 ? 'text-red-500' : 'text-gray-900 dark:text-white'}`}>
                        {diff > 0 ? '+' : ''}{diff}
                      </td>
                      <td className="py-3 px-2 text-center">
                        <Text variant="primary">{team.spirit_avg.toFixed(1)}</Text>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  );
}
