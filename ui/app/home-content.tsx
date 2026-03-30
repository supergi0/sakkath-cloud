'use client';

import { useEffect, useState } from 'react';
import Image from 'next/image';
import Link from 'next/link';
import { Calendar, Clock, LayoutGrid, MapPin, Target, Trophy, Users } from 'lucide-react';
import { Text } from './components/Text';

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

interface DashboardPreferences {
  division: 'open' | 'women';
  sortBy: SortBy;
}

const DASHBOARD_PREFS_KEY = 'sakkath:dashboard:preferences';

export function HomeContent() {
  const [mounted, setMounted] = useState(false);
  const [division, setDivision] = useState<'open' | 'women'>('open');
  const [sortBy, setSortBy] = useState<SortBy>('game');
  const [openStandings, setOpenStandings] = useState<TeamStanding[]>([]);
  const [womenStandings, setWomenStandings] = useState<TeamStanding[]>([]);
  const [stats, setStats] = useState<Stats>({ teams: 0, players: 0, points: 0, games: 0, fields: 0 });
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setMounted(true);

    const savedPreferences = localStorage.getItem(DASHBOARD_PREFS_KEY);
    if (savedPreferences) {
      try {
        const parsed: DashboardPreferences = JSON.parse(savedPreferences);
        if (parsed.division === 'open' || parsed.division === 'women') {
          setDivision(parsed.division);
        }
        if (parsed.sortBy === 'game' || parsed.sortBy === 'initial' || parsed.sortBy === 'spirit') {
          setSortBy(parsed.sortBy);
        }
      } catch {
      }
    }

    Promise.all([
      fetch(`${API_URL}/v1/standings?division=0`).then((response) => response.json()),
      fetch(`${API_URL}/v1/standings?division=1`).then((response) => response.json()),
      fetch(`${API_URL}/v1/stats`).then((response) => response.json()),
    ]).then(([open, women, statsData]) => {
      setOpenStandings(open);
      setWomenStandings(women);
      setStats(statsData);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  useEffect(() => {
    if (!mounted) return;
    const preferences: DashboardPreferences = { division, sortBy };
    localStorage.setItem(DASHBOARD_PREFS_KEY, JSON.stringify(preferences));
  }, [mounted, division, sortBy]);

  const sortStandings = (standings: TeamStanding[]) => {
    return [...standings].sort((a, b) => {
      switch (sortBy) {
        case 'initial':
          return a.init_rank - b.init_rank;
        case 'spirit':
          return b.spirit_avg - a.spirit_avg;
        case 'game':
        default: {
          const aWinPct = a.wins / (a.wins + a.losses || 1);
          const bWinPct = b.wins / (b.wins + b.losses || 1);
          if (bWinPct !== aWinPct) return bWinPct - aWinPct;
          return (b.points_for - b.points_against) - (a.points_for - a.points_against);
        }
      }
    });
  };

  const currentStandings = sortStandings(division === 'open' ? openStandings : womenStandings);

  if (!mounted) return null;

  return (
    <div className="min-h-screen overflow-x-hidden px-4 py-4 md:px-0">
      <div className="mx-auto max-w-7xl">
        <div className="mb-5 rounded-sm bg-white p-6 dark:bg-slate-900">
          <div className="md:grid md:grid-cols-[minmax(260px,320px)_1fr_minmax(220px,280px)] md:items-stretch md:gap-6">
            <div className="mb-6 flex w-full items-stretch justify-between gap-4 md:hidden">
              <Link href="/" className="flex min-w-0 flex-1 items-center justify-center p-2">
                <div className="relative h-[112px] w-full max-w-[260px] transition-opacity hover:opacity-80">
                  {mounted && (
                    <Image src="/sakkath.png" alt="Sakkath Ultimate Open" fill className="object-contain" priority />
                  )}
                </div>
              </Link>
              <div className="flex w-[132px] items-center justify-center rounded-sm border border-gray-200 bg-gray-100 p-4 text-center dark:border-slate-700 dark:bg-slate-800">
                <Text variant="secondary" className="text-sm font-medium leading-snug">
                  Title Sponsor
                </Text>
              </div>
            </div>

            <div className="hidden min-h-[220px] items-center justify-center p-2 md:flex">
              <Link href="/" className="relative h-[210px] w-full transition-opacity hover:opacity-80">
                {mounted && (
                  <Image src="/sakkath.png" alt="Sakkath Ultimate Open" fill className="object-contain" priority />
                )}
              </Link>
            </div>

            <div className="min-w-0 text-center">
              <Link href="/" className="inline-block transition-colors hover:text-gray-700 dark:hover:text-gray-200">
                <Text as="h1" variant="primary" className="mb-2 text-center text-2xl font-semibold">
                  Sakkath Ultimate Open 2026
                </Text>
              </Link>
              <div className="flex flex-wrap justify-center gap-x-4 gap-y-2 text-sm">
                <Text variant="secondary" className="flex items-center justify-center gap-1">
                  <Calendar className="h-4 w-4" />
                  May 22 - May 24, 2026
                </Text>
                <Text variant="secondary" className="flex items-center justify-center gap-1">
                  <MapPin className="h-4 w-4" />
                  Bangalore, India
                </Text>
                <Text variant="secondary" className="hidden sm:flex items-center justify-center gap-1">
                  <Clock className="h-4 w-4" />
                  GMT+5:30
                </Text>
              </div>

              <div className="mt-6 flex flex-wrap justify-center gap-3 border-t border-gray-200 pt-6 dark:border-slate-700">
                <div className="hidden w-full flex-wrap justify-center gap-3 sm:flex">
                  {[
                    { icon: Users, value: stats.teams.toString(), label: 'Teams' },
                    { icon: Users, value: stats.players.toString(), label: 'Players' },
                    { icon: Target, value: stats.points.toString(), label: 'Points' },
                    { icon: Trophy, value: stats.games.toString(), label: 'Games' },
                    { icon: LayoutGrid, value: stats.fields.toString(), label: 'Fields' },
                    { icon: Calendar, value: '3', label: 'Days' },
                  ].map((stat) => (
                    <div key={stat.label} className="flex items-center gap-2 rounded bg-gray-100 px-4 py-2 dark:bg-slate-800">
                      <stat.icon className="h-4 w-4 text-gray-500 dark:text-gray-400" />
                      <Text variant="primary" className="font-bold">{stat.value}</Text>
                      <Text variant="secondary" className="text-sm">{stat.label}</Text>
                    </div>
                  ))}
                </div>

                <div className="flex w-full flex-col gap-3 sm:hidden">
                  <div className="flex gap-3">
                    {[
                      { icon: Users, value: stats.teams.toString(), label: 'Teams' },
                      { icon: Users, value: stats.players.toString(), label: 'Players' },
                    ].map((stat) => (
                      <div key={stat.label} className="flex flex-1 items-center gap-2 rounded bg-gray-100 px-3 py-2 dark:bg-slate-800">
                        <stat.icon className="h-4 w-4 text-gray-500 dark:text-gray-400" />
                        <Text variant="primary" className="text-sm font-bold">{stat.value}</Text>
                        <Text variant="secondary" className="text-xs">{stat.label}</Text>
                      </div>
                    ))}
                  </div>
                  <div className="flex gap-3">
                    {[
                      { icon: Target, value: stats.points.toString(), label: 'Points' },
                      { icon: Trophy, value: stats.games.toString(), label: 'Games' },
                    ].map((stat) => (
                      <div key={stat.label} className="flex flex-1 items-center gap-2 rounded bg-gray-100 px-3 py-2 dark:bg-slate-800">
                        <stat.icon className="h-4 w-4 text-gray-500 dark:text-gray-400" />
                        <Text variant="primary" className="text-sm font-bold">{stat.value}</Text>
                        <Text variant="secondary" className="text-xs">{stat.label}</Text>
                      </div>
                    ))}
                  </div>
                  <div className="flex gap-3">
                    {[
                      { icon: LayoutGrid, value: stats.fields.toString(), label: 'Fields' },
                      { icon: Calendar, value: '3', label: 'Days' },
                    ].map((stat) => (
                      <div key={stat.label} className="flex flex-1 items-center gap-2 rounded bg-gray-100 px-3 py-2 dark:bg-slate-800">
                        <stat.icon className="h-4 w-4 text-gray-500 dark:text-gray-400" />
                        <Text variant="primary" className="text-sm font-bold">{stat.value}</Text>
                        <Text variant="secondary" className="text-xs">{stat.label}</Text>
                      </div>
                    ))}
                  </div>
                </div>
              </div>


            </div>

            <div className="hidden min-h-[220px] items-center justify-center rounded-sm border border-gray-200 bg-gray-100 p-6 text-center dark:border-slate-700 dark:bg-slate-800 md:flex">
              <Text variant="secondary" className="text-xl font-medium leading-snug">
                Title Sponsor
              </Text>
            </div>
          </div>
        </div>

        <div className="rounded-sm bg-white p-6 dark:bg-slate-900">
          <div className="mb-4 hidden items-center justify-between gap-3 sm:flex">
            <div className="flex items-center gap-4">
              <Text as="h2" variant="primary" className="text-xl">
                Standings
              </Text>
              <div className="flex gap-1">
                {(['game', 'initial', 'spirit'] as SortBy[]).map((selection) => (
                  <button
                    key={selection}
                    onClick={() => setSortBy(selection)}
                    className={`rounded px-3 py-2 text-sm transition-colors ${sortBy === selection ? 'bg-blue-900 text-white' : 'bg-gray-200 text-gray-600 dark:bg-slate-700 dark:text-gray-300'}`}
                  >
                    {selection.charAt(0).toUpperCase() + selection.slice(1)}
                  </button>
                ))}
              </div>
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => setDivision('open')}
                className={`rounded px-4 py-2 text-sm font-medium transition-colors ${division === 'open' ? 'bg-blue-900 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300 dark:bg-slate-800 dark:text-gray-300 dark:hover:bg-slate-700'}`}
              >
                Open Division
              </button>
              <button
                onClick={() => setDivision('women')}
                className={`rounded px-4 py-2 text-sm font-medium transition-colors ${division === 'women' ? 'bg-blue-900 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300 dark:bg-slate-800 dark:text-gray-300 dark:hover:bg-slate-700'}`}
              >
                Women Division
              </button>
            </div>
          </div>

          <div className="mb-4 flex flex-col gap-3 sm:hidden">
            <Text as="h2" variant="primary" className="text-xl">
              Standings
            </Text>
            <div className="flex gap-2">
              {(['game', 'initial', 'spirit'] as SortBy[]).map((selection) => (
                <button
                  key={selection}
                  onClick={() => setSortBy(selection)}
                  className={`flex-1 rounded px-3 py-2 text-sm transition-colors ${sortBy === selection ? 'bg-blue-900 text-white' : 'bg-gray-200 text-gray-600 dark:bg-slate-700 dark:text-gray-300'}`}
                >
                  {selection === 'game' ? 'Game' : selection === 'initial' ? 'Initial' : 'Spirit'}
                </button>
              ))}
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => setDivision('open')}
                className={`flex-1 rounded px-3 py-2.5 text-sm font-medium transition-colors ${division === 'open' ? 'bg-blue-900 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300 dark:bg-slate-800 dark:text-gray-300 dark:hover:bg-slate-700'}`}
              >
                Open
              </button>
              <button
                onClick={() => setDivision('women')}
                className={`flex-1 rounded px-3 py-2.5 text-sm font-medium transition-colors ${division === 'women' ? 'bg-blue-900 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300 dark:bg-slate-800 dark:text-gray-300 dark:hover:bg-slate-700'}`}
              >
                Women
              </button>
            </div>
          </div>

          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-gray-200 dark:border-slate-700">
                  <th className="px-2 py-3 text-left font-medium text-gray-500 dark:text-gray-400">#</th>
                  <th className="px-2 py-3 text-left font-medium text-gray-500 dark:text-gray-400">Team</th>
                  <th className="px-2 py-3 text-center font-medium text-gray-500 dark:text-gray-400">P</th>
                  <th className="px-2 py-3 text-center font-medium text-gray-500 dark:text-gray-400">W</th>
                  <th className="px-2 py-3 text-center font-medium text-gray-500 dark:text-gray-400">L</th>
                  <th className="px-2 py-3 text-center font-medium text-gray-500 dark:text-gray-400">PF</th>
                  <th className="px-2 py-3 text-center font-medium text-gray-500 dark:text-gray-400">PA</th>
                  <th className="px-2 py-3 text-center font-medium text-gray-500 dark:text-gray-400">Diff</th>
                  <th className="px-2 py-3 text-center font-medium text-gray-500 dark:text-gray-400">Spirit</th>
                </tr>
              </thead>
              <tbody>
                {!loading && currentStandings.map((team, index) => {
                  const diff = team.points_for - team.points_against;
                  const initial = team.name.charAt(0).toUpperCase();

                  return (
                    <tr key={team.id} className="border-b border-gray-200 hover:opacity-80 dark:border-slate-700">
                      <td className="px-2 py-3">
                        <Text variant="primary" className="font-medium">{index + 1}</Text>
                      </td>
                      <td className="px-2 py-3">
                        <Link href={`/teams?team_id=${team.id}`} className="flex items-center gap-2 hover:underline">
                          <div className="flex h-7 w-7 flex-shrink-0 items-center justify-center overflow-hidden rounded-full bg-blue-900 text-xs font-bold text-white sm:h-6 sm:w-6">
                            {team.small_logo ? (
                              <img src={team.small_logo} alt={team.name} className="h-full w-full object-cover" />
                            ) : (
                              initial
                            )}
                          </div>
                          <Text variant="primary" className="font-medium truncate max-w-[100px] sm:max-w-[180px]">{team.name}</Text>
                        </Link>
                      </td>
                      <td className="px-2 py-3 text-center">
                        <Text variant="secondary">{team.wins + team.losses}</Text>
                      </td>
                      <td className="px-2 py-3 text-center text-green-500">{team.wins}</td>
                      <td className="px-2 py-3 text-center text-red-500">{team.losses}</td>
                      <td className="px-2 py-3 text-center">
                        <Text variant="primary">{team.points_for}</Text>
                      </td>
                      <td className="px-2 py-3 text-center">
                        <Text variant="primary">{team.points_against}</Text>
                      </td>
                      <td className={`px-2 py-3 text-center font-medium ${diff > 0 ? 'text-green-500' : diff < 0 ? 'text-red-500' : 'text-gray-900 dark:text-white'}`}>
                        {diff > 0 ? '+' : ''}{diff}
                      </td>
                      <td className="px-2 py-3 text-center">
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