'use client';

import Image from 'next/image';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { ChevronDown, ExternalLink, GripVertical, Shield, X } from 'lucide-react';

import { useAuth } from '../auth-provider';
import { Text } from '../components/Text';
import { Toast } from '../components/Toast';
import { apiUrl } from '../lib/api';
import { getTeamAbbreviation } from '../lib/team-name';

type CellStatus = 'empty' | 'upcoming' | 'live' | 'done';

type RowDraftMap = Record<string, { start_time: string; end_time: string }>;

interface ScheduleGridCell {
  field_index: number;
  field_label: string;
  field_name: string;
  slot_code: string | null;
  division: number | null;
  match_type: number | null;
  match_id: number | null;
  data: [number, number, number, number, number] | null;
  seed_ranks: [number, number] | null;
  stream_url: string | null;
  possession: number | null;
  status: CellStatus;
  clickable: boolean;
  movable: boolean;
}

interface ScheduleTeam {
  id: number;
  name: string;
  abbreviation: string | null;
  division: number;
  small_logo: string | null;
}

interface ScheduleGridRow {
  key: string;
  day_key: string;
  day_label: string;
  label: string;
  start_time: string;
  end_time: string;
  cells: ScheduleGridCell[];
}

interface ScheduleGridResponse {
  rows: ScheduleGridRow[];
}

interface ScheduleTeamsResponse {
  teams: ScheduleTeam[];
}

interface DragMatch {
  matchId: number;
  division: number;
  matchType: number;
}

const DAY_ORDER = ['fri', 'sat', 'sun'] as const;
const DAY_LABELS: Record<(typeof DAY_ORDER)[number], string> = {
  fri: 'Friday',
  sat: 'Saturday',
  sun: 'Sunday',
};

function formatCompactTime(value: string) {
  return value.replace(/^0/, '').replace(':', '.');
}

function getRowTitle(label: string) {
  const title = label.split(' · ')[0] ?? label;
  return title === 'Playoff 2' ? 'Final' : title;
}

function getStageLabel(cell: ScheduleGridCell) {
  const divisionLabel = cell.division === 1 ? 'Women' : 'Open';
  const round = cell.data?.[2] ?? cell.match_type;

  if (round === 1002) {
    return `${divisionLabel} F`;
  }
  if (round === 1001) {
    return `${divisionLabel} P1`;
  }
  if (round !== null) {
    return `${divisionLabel} R${round}`;
  }
  return cell.slot_code ?? '';
}

function getStatusLabel(status: CellStatus) {
  if (status === 'done') {
    return 'Ended';
  }
  if (status === 'live') {
    return 'Live';
  }
  return 'Pending';
}

function getStatusDotClass(status: CellStatus) {
  if (status === 'done') {
    return 'bg-slate-400 dark:bg-slate-500';
  }
  if (status === 'live') {
    return 'bg-red-500 animate-pulse shadow-[0_0_0_3px_rgba(239,68,68,0.16)]';
  }
  return 'bg-amber-400 shadow-[0_0_0_3px_rgba(251,191,36,0.16)]';
}

function getSeedLabel(cell: ScheduleGridCell) {
  if (!cell.seed_ranks) {
    return null;
  }
  return `${cell.seed_ranks[0]} v ${cell.seed_ranks[1]}`;
}

function TeamLogo({ name, logo }: { name: string | null; logo: string | null }) {
  const initial = (name ?? '?').trim().charAt(0).toUpperCase() || '?';

  return (
    <div className="flex h-12 w-12 items-center justify-center overflow-hidden rounded-full bg-slate-900 text-sm font-semibold text-white dark:bg-slate-700">
      {logo ? <Image src={logo} alt="" width={48} height={48} unoptimized className="h-full w-full object-cover" /> : initial}
    </div>
  );
}

function InlineTeamLogo({ name, logo }: { name: string | null; logo: string | null }) {
  const initial = (name ?? '?').trim().charAt(0).toUpperCase() || '?';

  return (
    <div className="flex h-5 w-5 shrink-0 items-center justify-center overflow-hidden rounded-full bg-slate-900 text-[10px] font-semibold text-white dark:bg-slate-700">
      {logo ? <Image src={logo} alt="" width={20} height={20} unoptimized className="h-full w-full object-cover" /> : initial}
    </div>
  );
}

function ScheduleDetailModal({
  row,
  cell,
  teamsById,
  onClose,
}: {
  row: ScheduleGridRow;
  cell: ScheduleGridCell;
  teamsById: Record<number, ScheduleTeam>;
  onClose: () => void;
}) {
  const stageLabel = getStageLabel(cell);
  const hasMatch = cell.match_id !== null;
  const t1Id = cell.data?.[0] ?? null;
  const t2Id = cell.data?.[1] ?? null;
  const t1Team = t1Id !== null ? teamsById[t1Id] : undefined;
  const t2Team = t2Id !== null ? teamsById[t2Id] : undefined;
  const t1Score = cell.data?.[3] ?? 0;
  const t2Score = cell.data?.[4] ?? 0;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 px-4 backdrop-blur-sm" onClick={onClose}>
      <div
        className="w-full max-w-lg rounded-2xl border border-gray-200 bg-white p-5 shadow-xl dark:border-slate-700 dark:bg-slate-900"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-4">
          <div>
            <p className="text-[11px] font-semibold uppercase tracking-[0.22em] text-gray-500 dark:text-slate-400">
              {stageLabel}
            </p>
            <Text as="h2" variant="primary" className="mt-2 text-lg font-semibold">
              {getRowTitle(row.label)}
            </Text>
            <div className="mt-3 flex flex-wrap gap-2 text-xs text-gray-600 dark:text-slate-300">
              <span className="rounded-full bg-gray-100 px-3 py-1 dark:bg-slate-800">
                {row.start_time} - {row.end_time}
              </span>
              <span className="rounded-full bg-gray-100 px-3 py-1 dark:bg-slate-800">{cell.field_name}</span>
              <span className="rounded-full bg-gray-100 px-3 py-1 dark:bg-slate-800">{getStatusLabel(cell.status)}</span>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-full p-2 text-gray-500 transition hover:bg-gray-100 hover:text-gray-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-white"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {hasMatch ? (
          <div className="mt-5 space-y-3">
            <div className="rounded-2xl border border-gray-200 p-4 dark:border-slate-700">
              <div className="flex items-center justify-between gap-3">
                <div className="flex min-w-0 items-center gap-3">
                  <TeamLogo name={t1Team?.name ?? null} logo={t1Team?.small_logo ?? null} />
                  <Text variant="primary" className="min-w-0 text-base font-semibold break-words">
                    {t1Team?.name ?? 'TBD'}
                  </Text>
                </div>
                <Text variant="primary" className="text-2xl font-semibold">
                  {t1Score}
                </Text>
              </div>
            </div>

            <div className="rounded-2xl border border-gray-200 p-4 dark:border-slate-700">
              <div className="flex items-center justify-between gap-3">
                <div className="flex min-w-0 items-center gap-3">
                  <TeamLogo name={t2Team?.name ?? null} logo={t2Team?.small_logo ?? null} />
                  <Text variant="primary" className="min-w-0 text-base font-semibold break-words">
                    {t2Team?.name ?? 'TBD'}
                  </Text>
                </div>
                <Text variant="primary" className="text-2xl font-semibold">
                  {t2Score}
                </Text>
              </div>
            </div>
          </div>
        ) : (
          <div className="mt-5 rounded-2xl border border-dashed border-gray-200 p-6 text-center dark:border-slate-700">
            <Text variant="secondary" className="text-sm">
              Matchup is not known yet.
            </Text>
            <Text variant="primary" className="mt-2 text-base font-semibold">
              TBD
            </Text>
          </div>
        )}

        <div className="mt-5 flex flex-wrap justify-end gap-3">
          {cell.stream_url ? (
            <a
              href={cell.stream_url}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-2 rounded-lg bg-gray-100 px-4 py-2 text-sm font-medium text-gray-700 transition hover:bg-gray-200 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
            >
              <ExternalLink className="h-4 w-4" />
              Watch
            </a>
          ) : null}
          {cell.match_id !== null ? (
            <a
              href={`/matches?match_id=${cell.match_id}`}
              className="inline-flex items-center gap-2 rounded-lg bg-slate-900 px-4 py-2 text-sm font-semibold text-white transition hover:bg-slate-800 dark:bg-slate-100 dark:text-slate-950 dark:hover:bg-white"
            >
              <ExternalLink className="h-4 w-4" />
              Match page
            </a>
          ) : null}
        </div>
      </div>
    </div>
  );
}

export default function SchedulePage() {
  const { isSuperAdmin, token } = useAuth();
  const [rows, setRows] = useState<ScheduleGridRow[]>([]);
  const [teamsById, setTeamsById] = useState<Record<number, ScheduleTeam>>({});
  const [loading, setLoading] = useState(true);
  const [drafts, setDrafts] = useState<RowDraftMap>({});
  const [savingRow, setSavingRow] = useState<string | null>(null);
  const [movingMatchId, setMovingMatchId] = useState<number | null>(null);
  const [draggedMatch, setDraggedMatch] = useState<DragMatch | null>(null);
  const [toastMessage, setToastMessage] = useState('');
  const [toastOpen, setToastOpen] = useState(false);
  const [selectedDay, setSelectedDay] = useState<(typeof DAY_ORDER)[number]>('fri');
  const [selectedCell, setSelectedCell] = useState<{ row: ScheduleGridRow; cell: ScheduleGridCell } | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [statusLabelCellKey, setStatusLabelCellKey] = useState<string | null>(null);

  const showToast = useCallback((message: string) => {
    setToastMessage(message);
    setToastOpen(true);
  }, []);

  const fetchGrid = useCallback(async () => {
    try {
      const response = await fetch(apiUrl('/v1/schedule/grid'));
      if (!response.ok) {
        throw new Error('Unable to load schedule');
      }
      const data: ScheduleGridResponse = await response.json();
      setRows(data.rows);
      setDrafts((current) => {
        const next = { ...current };
        for (const row of data.rows) {
          if (!next[row.key]) {
            next[row.key] = { start_time: row.start_time, end_time: row.end_time };
          }
        }
        return next;
      });
    } catch {
      showToast('Unable to load the schedule right now.');
      throw new Error('schedule-grid-load-failed');
    }
  }, [showToast]);

  const fetchTeams = useCallback(async () => {
    try {
      const response = await fetch(apiUrl('/v1/schedule/teams'));
      if (!response.ok) {
        throw new Error('Unable to load teams');
      }
      const data: ScheduleTeamsResponse = await response.json();
      setTeamsById(
        Object.fromEntries(data.teams.map((team) => [team.id, team])) as Record<number, ScheduleTeam>,
      );
    } catch {
      showToast('Unable to load team info right now.');
      throw new Error('schedule-teams-load-failed');
    }

  }, [showToast]);

  useEffect(() => {
    let active = true;

    const loadInitial = async () => {
      setLoading(true);
      try {
        await Promise.all([fetchTeams(), fetchGrid()]);
      } finally {
        if (active) {
          setLoading(false);
        }
      }
    };

    loadInitial().catch(() => undefined);
    const interval = setInterval(() => {
      fetchGrid().catch(() => undefined);
    }, 4000);

    return () => {
      active = false;
      clearInterval(interval);
    };
  }, [fetchGrid, fetchTeams]);

  const rowsByDay = useMemo(() => {
    const grouped: Record<string, ScheduleGridRow[]> = { fri: [], sat: [], sun: [] };
    for (const row of rows) {
      if (!grouped[row.day_key]) {
        grouped[row.day_key] = [];
      }
      grouped[row.day_key].push(row);
    }
    return grouped;
  }, [rows]);

  const updateDraft = (rowKey: string, field: 'start_time' | 'end_time', value: string) => {
    setDrafts((current) => ({
      ...current,
      [rowKey]: {
        start_time: current[rowKey]?.start_time ?? '',
        end_time: current[rowKey]?.end_time ?? '',
        [field]: value,
      },
    }));
  };

  const saveRow = async (rowKey: string) => {
    if (!token) {
      showToast('Super admin login is required.');
      return;
    }

    const draft = drafts[rowKey];
    if (!draft) {
      return;
    }

    try {
      setSavingRow(rowKey);
      const response = await fetch(apiUrl(`/v1/super/schedule/rows/${rowKey}`), {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify(draft),
      });
      if (!response.ok) {
        throw new Error('Unable to save row time');
      }
      showToast('Row timing updated.');
      await fetchGrid();
    } catch {
      showToast('Row timing update was rejected.');
    } finally {
      setSavingRow(null);
    }
  };

  const canDropIntoCell = (cell: ScheduleGridCell) => {
    if (!draggedMatch || !cell.slot_code || cell.division === null || cell.match_type === null) {
      return false;
    }
    if (cell.match_id === draggedMatch.matchId) {
      return false;
    }
    if (cell.division !== draggedMatch.division || cell.match_type !== draggedMatch.matchType) {
      return false;
    }
    return cell.status !== 'live' && cell.status !== 'done';
  };

  const moveMatch = async (rowKey: string, fieldIndex: number) => {
    if (!draggedMatch || !token) {
      return;
    }

    try {
      setMovingMatchId(draggedMatch.matchId);
      const response = await fetch(apiUrl(`/v1/super/schedule/matches/${draggedMatch.matchId}/slot`), {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ row_key: rowKey, field_index: fieldIndex }),
      });
      if (!response.ok) {
        throw new Error('Unable to move match');
      }
      showToast('Match moved.');
      await fetchGrid();
    } catch {
      showToast('Match move was rejected.');
    } finally {
      setMovingMatchId(null);
      setDraggedMatch(null);
    }
  };

  const renderCell = (row: ScheduleGridRow, cell: ScheduleGridCell) => {
    const hasMatch = cell.match_id !== null;
    const interactive = Boolean(cell.slot_code);
    const draggable = Boolean(
      isSuperAdmin && hasMatch && cell.movable && cell.division !== null && cell.match_type !== null,
    );
    const droppable = isSuperAdmin && canDropIntoCell(cell);
    const stageLabel = getStageLabel(cell);
    const seedLabel = getSeedLabel(cell);
    const cellKey = `${row.key}-${cell.field_index}`;
    const t1Id = cell.data?.[0] ?? null;
    const t2Id = cell.data?.[1] ?? null;
    const t1Team = t1Id !== null ? teamsById[t1Id] : undefined;
    const t2Team = t2Id !== null ? teamsById[t2Id] : undefined;
    const t1Short = t1Team ? getTeamAbbreviation(t1Team.name, t1Team.abbreviation) : '';
    const t2Short = t2Team ? getTeamAbbreviation(t2Team.name, t2Team.abbreviation) : '';
    const t1Score = cell.data?.[3] ?? 0;
    const t2Score = cell.data?.[4] ?? 0;

    const className = hasMatch
      ? cell.status === 'upcoming'
        ? 'border-gray-200 bg-gray-100 text-gray-500 dark:border-slate-700 dark:bg-slate-900/70 dark:text-slate-400'
        : cell.status === 'live'
          ? 'border-red-200 bg-red-50 text-gray-900 dark:border-red-900/70 dark:bg-red-950/40 dark:text-white'
          : 'border-emerald-200 bg-emerald-50 text-gray-900 dark:border-emerald-900/70 dark:bg-emerald-950/40 dark:text-white'
      : cell.slot_code
        ? 'border-dashed border-gray-200 bg-white/80 text-gray-500 dark:border-slate-700 dark:bg-slate-900/40 dark:text-slate-400'
        : 'border-transparent bg-transparent text-transparent';

    return (
      <td
        key={`${row.key}-${cell.field_index}`}
        className="w-[92px] border-r border-t border-gray-200 p-0 align-top dark:border-slate-700 sm:w-[104px]"
        onDragOver={(event) => {
          if (droppable) {
            event.preventDefault();
            event.dataTransfer.dropEffect = 'move';
          }
        }}
        onDrop={(event) => {
          event.preventDefault();
          if (droppable) {
            moveMatch(row.key, cell.field_index).catch(() => undefined);
          }
        }}
      >
        <div
          draggable={draggable}
          onDragStart={(event) => {
            if (!draggable || cell.match_id === null || cell.division === null || cell.match_type === null) {
              return;
            }
            event.dataTransfer.effectAllowed = 'move';
            event.dataTransfer.setData('text/plain', String(cell.match_id));
            setIsDragging(true);
            setDraggedMatch({
              matchId: cell.match_id,
              division: cell.division,
              matchType: cell.match_type,
            });
          }}
          onDragEnd={() => {
            setDraggedMatch(null);
            setTimeout(() => setIsDragging(false), 0);
          }}
          onClick={() => {
            if (interactive && !isDragging) {
              setStatusLabelCellKey(null);
              setSelectedCell({ row, cell });
            }
          }}
          className={`flex h-full min-h-[88px] flex-col justify-between rounded-none border-0 px-1 py-1.5 transition sm:px-1.5 ${className} ${
            interactive ? 'cursor-pointer hover:shadow-sm' : ''
          } ${droppable ? 'ring-2 ring-sky-500/60' : ''}`}
        >
          {cell.slot_code ? (
            <div className="flex items-start justify-between gap-2">
              <div className="min-w-0">
                <p className="truncate text-[10px] font-semibold uppercase tracking-[0.14em] sm:text-[11px]">{stageLabel}</p>
              </div>
              <div className="relative flex shrink-0 items-center gap-1.5">
                {draggable ? <GripVertical className="h-4 w-4 shrink-0 text-gray-400 dark:text-slate-500" /> : null}
                {hasMatch ? (
                  <>
                    <button
                      type="button"
                      aria-label={`Show ${getStatusLabel(cell.status)} status`}
                      onClick={(event) => {
                        event.stopPropagation();
                        setStatusLabelCellKey((current) => (current === cellKey ? null : cellKey));
                      }}
                      className="flex h-4 w-4 items-center justify-center rounded-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sky-500/70"
                    >
                      <span className={`h-2.5 w-2.5 rounded-full ${getStatusDotClass(cell.status)}`} />
                    </button>
                    {statusLabelCellKey === cellKey ? (
                      <span className="absolute right-0 top-full z-10 mt-1 whitespace-nowrap rounded-md bg-slate-950 px-2 py-1 text-[9px] font-semibold uppercase tracking-[0.12em] text-white shadow-lg dark:bg-slate-100 dark:text-slate-950">
                        {getStatusLabel(cell.status)}
                      </span>
                    ) : null}
                  </>
                ) : null}
              </div>
            </div>
          ) : null}

          {hasMatch ? (
            <div className="mt-1.5 space-y-1.5">
              <div className="space-y-1.5">
                <div className="flex min-w-0 items-center justify-between gap-1.5">
                  <div className="flex min-w-0 flex-1 items-center gap-1.5">
                    <InlineTeamLogo name={t1Team?.name ?? null} logo={t1Team?.small_logo ?? null} />
                    <p className="min-w-0 flex-1 truncate text-[14px] font-semibold leading-tight sm:text-[15px]">{t1Short}</p>
                  </div>
                  <span className="shrink-0 text-[14px] font-semibold leading-none tabular-nums sm:text-[15px]">{t1Score}</span>
                </div>
                <div className="flex min-w-0 items-center justify-between gap-1.5">
                  <div className="flex min-w-0 flex-1 items-center gap-1.5">
                    <InlineTeamLogo name={t2Team?.name ?? null} logo={t2Team?.small_logo ?? null} />
                    <p className="min-w-0 flex-1 truncate text-[14px] font-semibold leading-tight sm:text-[15px]">{t2Short}</p>
                  </div>
                  <span className="shrink-0 text-[14px] font-semibold leading-none tabular-nums sm:text-[15px]">{t2Score}</span>
                </div>
              </div>
              <div className="space-y-0.5 text-[10px] uppercase tracking-[0.12em]">
                {seedLabel ? (
                  <p className="text-[11px] font-semibold tracking-[0.1em] text-gray-500 dark:text-slate-400 sm:text-[12px]">{seedLabel}</p>
                ) : null}
                {movingMatchId === cell.match_id ? <p className="text-[9px]">Saving</p> : null}
              </div>
            </div>
          ) : cell.slot_code ? (
            <div className="mt-3">
              <p className="text-sm font-medium text-gray-400 dark:text-slate-500">TBD</p>
            </div>
          ) : null}
        </div>
      </td>
    );
  };

  if (loading) {
    return (
      <div className="bg-gray-100 dark:bg-slate-950">
        <div className="sticky top-14 z-30 bg-white shadow-sm dark:bg-slate-900 md:static md:shadow-none">
          <div className="px-4 py-4 sm:mx-auto sm:max-w-7xl sm:px-4">
            <Text as="h1" variant="primary" className="text-xl">
              Schedule
            </Text>
          </div>
        </div>
        <div className="px-4 py-8 sm:mx-auto sm:max-w-7xl sm:px-4">
          <Text variant="secondary">Loading...</Text>
        </div>
      </div>
    );
  }

  const visibleRows = rowsByDay[selectedDay] ?? [];

  return (
    <div className="bg-gray-100 dark:bg-slate-950">
      <div className="sticky top-14 z-30 bg-white shadow-sm dark:bg-slate-900 md:static md:shadow-none">
        <div className="px-4 py-4 sm:mx-auto sm:max-w-7xl sm:px-4">
          <div className="flex items-center justify-between gap-3">
            <Text as="h1" variant="primary" className="text-xl">
              Schedule
            </Text>
            <div className="flex items-center gap-2">
              <div className="relative w-[148px] shrink-0 sm:w-[160px]">
                <select
                  value={selectedDay}
                  onChange={(event) => setSelectedDay(event.target.value as (typeof DAY_ORDER)[number])}
                  className="w-full appearance-none rounded bg-gray-100 px-3 py-2 pr-8 text-sm text-gray-700 dark:bg-slate-800 dark:text-gray-300"
                >
                  {DAY_ORDER.map((dayKey) => (
                    <option key={dayKey} value={dayKey}>
                      {DAY_LABELS[dayKey]}
                    </option>
                  ))}
                </select>
                <ChevronDown className="pointer-events-none absolute right-2 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-500" />
              </div>
              {isSuperAdmin ? (
                <div className="inline-flex items-center gap-2 rounded bg-sky-50 px-3 py-2 text-xs font-semibold uppercase tracking-[0.16em] text-sky-700 dark:bg-sky-950/40 dark:text-sky-300">
                  <Shield className="h-4 w-4" />
                  Super
                </div>
              ) : null}
            </div>
          </div>
        </div>
      </div>

      <div className="pb-2 sm:mx-auto sm:max-w-7xl sm:px-4 sm:py-4">
        <div className="overflow-x-auto border-y border-gray-200 bg-white dark:border-slate-700 dark:bg-slate-900 sm:rounded-sm sm:border">
          <table className="w-full min-w-[452px] border-separate border-spacing-0 text-xs sm:min-w-[510px] sm:text-sm">
            <thead>
              <tr className="bg-white dark:bg-slate-900">
                <th className="sticky left-0 z-20 w-[84px] border-r border-gray-200 bg-white px-2 py-3 text-left font-medium text-gray-500 dark:border-slate-700 dark:bg-slate-900 dark:text-gray-400 sm:w-[96px]">
                  Time
                </th>
                {[1, 2, 3, 4].map((fieldIndex) => (
                  <th key={fieldIndex} className="border-r border-gray-200 px-2 py-3 text-left font-medium text-gray-500 dark:border-slate-700 dark:text-gray-400">
                    G{fieldIndex}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {visibleRows.map((row) => {
                const draft = drafts[row.key] ?? {
                  start_time: row.start_time,
                  end_time: row.end_time,
                };
                const changed = draft.start_time !== row.start_time || draft.end_time !== row.end_time;

                return (
                  <tr key={row.key} className="align-top">
                    <td className="sticky left-0 z-10 min-w-[84px] border-r border-t border-gray-200 bg-white p-0 align-middle dark:border-slate-700 dark:bg-slate-900 sm:w-[96px]">
                      <div className={`min-h-[88px] px-2 py-3 ${isSuperAdmin ? 'flex h-full items-start justify-between gap-2' : 'flex h-full flex-col justify-center'}`}>
                        <div className="space-y-0.5">
                          <Text variant="primary" className="whitespace-nowrap text-sm font-medium leading-tight text-gray-500 dark:text-gray-400 sm:text-[15px]">
                            {getRowTitle(row.label)}
                          </Text>
                          <div className="space-y-0 text-sm leading-tight text-gray-500 dark:text-gray-400 sm:text-[15px]">
                            <p>St {formatCompactTime(row.start_time)}</p>
                            <p>Ed {formatCompactTime(row.end_time)}</p>
                          </div>
                        </div>
                        {isSuperAdmin ? (
                          <div className="space-y-1.5">
                            <input
                              type="time"
                              value={draft.start_time}
                              onChange={(event) => updateDraft(row.key, 'start_time', event.target.value)}
                              className="w-full rounded border border-gray-300 bg-white px-2 py-1 text-[11px] text-gray-900 dark:border-slate-700 dark:bg-slate-950 dark:text-white"
                            />
                            <input
                              type="time"
                              value={draft.end_time}
                              onChange={(event) => updateDraft(row.key, 'end_time', event.target.value)}
                              className="w-full rounded border border-gray-300 bg-white px-2 py-1 text-[11px] text-gray-900 dark:border-slate-700 dark:bg-slate-950 dark:text-white"
                            />
                            <button
                              type="button"
                              disabled={!changed || savingRow === row.key}
                              onClick={() => saveRow(row.key).catch(() => undefined)}
                              className="w-full rounded bg-sky-600 px-2 py-1 text-[10px] font-semibold uppercase tracking-[0.14em] text-white disabled:cursor-not-allowed disabled:bg-gray-300 dark:disabled:bg-slate-700"
                            >
                              {savingRow === row.key ? 'Saving' : 'Save'}
                            </button>
                          </div>
                        ) : null}
                      </div>
                    </td>
                    {row.cells.map((cell) => renderCell(row, cell))}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>

      {selectedCell ? (
        <ScheduleDetailModal
          row={selectedCell.row}
          cell={selectedCell.cell}
          teamsById={teamsById}
          onClose={() => setSelectedCell(null)}
        />
      ) : null}

      <Toast message={toastMessage} isOpen={toastOpen} onClose={() => setToastOpen(false)} />
    </div>
  );
}
