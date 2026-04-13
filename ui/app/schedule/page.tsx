'use client';

import Image from 'next/image';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { ChevronDown, ExternalLink, GripVertical, Shield, X } from 'lucide-react';

import { useAuth } from '../auth-provider';
import { Text } from '../components/Text';
import { Toast } from '../components/Toast';
import { getTeamAbbreviation } from '../lib/team-name';

const API_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:9000';

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
  t1_id: number | null;
  t2_id: number | null;
  t1_name: string | null;
  t2_name: string | null;
  t1_abbreviation: string | null;
  t2_abbreviation: string | null;
  t1_score: number | null;
  t2_score: number | null;
  t1_small_logo: string | null;
  t2_small_logo: string | null;
  stream_url: string | null;
  possession: number | null;
  status: CellStatus;
  clickable: boolean;
  movable: boolean;
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
  if (cell.match_type === 1002) {
    return `${divisionLabel} F`;
  }
  if (cell.match_type === 1001) {
    return `${divisionLabel} P1`;
  }
  if (cell.match_type !== null) {
    return `${divisionLabel} R${cell.match_type}`;
  }
  return cell.slot_code ?? '';
}

function getStatusLabel(status: CellStatus) {
  if (status === 'done') {
    return 'Complete';
  }
  if (status === 'live') {
    return 'Live';
  }
  return 'Pending';
}

function TeamLogo({ name, logo }: { name: string | null; logo: string | null }) {
  const initial = (name ?? '?').trim().charAt(0).toUpperCase() || '?';

  return (
    <div className="flex h-12 w-12 items-center justify-center overflow-hidden rounded-full bg-slate-900 text-sm font-semibold text-white dark:bg-slate-700">
      {logo ? <Image src={logo} alt="" width={48} height={48} unoptimized className="h-full w-full object-cover" /> : initial}
    </div>
  );
}

function ScheduleDetailModal({
  row,
  cell,
  onClose,
}: {
  row: ScheduleGridRow;
  cell: ScheduleGridCell;
  onClose: () => void;
}) {
  const stageLabel = getStageLabel(cell);
  const hasMatch = cell.match_id !== null;

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
                  <TeamLogo name={cell.t1_name} logo={cell.t1_small_logo} />
                  <Text variant="primary" className="min-w-0 text-base font-semibold break-words">
                    {cell.t1_name}
                  </Text>
                </div>
                <Text variant="primary" className="text-2xl font-semibold">
                  {cell.t1_score ?? 0}
                </Text>
              </div>
            </div>

            <div className="rounded-2xl border border-gray-200 p-4 dark:border-slate-700">
              <div className="flex items-center justify-between gap-3">
                <div className="flex min-w-0 items-center gap-3">
                  <TeamLogo name={cell.t2_name} logo={cell.t2_small_logo} />
                  <Text variant="primary" className="min-w-0 text-base font-semibold break-words">
                    {cell.t2_name}
                  </Text>
                </div>
                <Text variant="primary" className="text-2xl font-semibold">
                  {cell.t2_score ?? 0}
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

  const showToast = (message: string) => {
    setToastMessage(message);
    setToastOpen(true);
  };

  const fetchGrid = useCallback(async (initial = false) => {
    if (initial) {
      setLoading(true);
    }

    try {
      const response = await fetch(`${API_URL}/v1/schedule/grid`);
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
    } finally {
      if (initial) {
        setLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    fetchGrid(true).catch(() => undefined);
    const interval = setInterval(() => {
      fetchGrid(false).catch(() => undefined);
    }, 4000);
    return () => clearInterval(interval);
  }, [fetchGrid]);

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
      const response = await fetch(`${API_URL}/v1/super/schedule/rows/${rowKey}`, {
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
      await fetchGrid(false);
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
      const response = await fetch(`${API_URL}/v1/super/schedule/matches/${draggedMatch.matchId}/slot`, {
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
      await fetchGrid(false);
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
    const t1Short = cell.t1_name ? getTeamAbbreviation(cell.t1_name, cell.t1_abbreviation) : '';
    const t2Short = cell.t2_name ? getTeamAbbreviation(cell.t2_name, cell.t2_abbreviation) : '';

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
        className="w-[116px] border-r border-t border-gray-200 align-top dark:border-slate-700 sm:w-[140px]"
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
              setSelectedCell({ row, cell });
            }
          }}
          className={`min-h-[88px] rounded-none border-0 px-1.5 py-2 transition sm:px-2 ${className} ${
            interactive ? 'cursor-pointer hover:shadow-sm' : ''
          } ${droppable ? 'ring-2 ring-sky-500/60' : ''}`}
        >
          {cell.slot_code ? (
            <div className="flex items-start justify-between gap-2">
              <div className="min-w-0">
                <p className="truncate text-[10px] font-semibold uppercase tracking-[0.16em]">{stageLabel}</p>
              </div>
              {draggable ? <GripVertical className="h-4 w-4 shrink-0 text-gray-400 dark:text-slate-500" /> : null}
            </div>
          ) : null}

          {hasMatch ? (
            <div className="mt-2 space-y-1">
              <div className="space-y-1">
                <div className="grid grid-cols-[minmax(0,1fr)_10px] items-center gap-x-0.5">
                  <p className="truncate text-[13px] font-semibold leading-tight sm:text-sm">{t1Short}</p>
                  <span className="text-[13px] font-semibold leading-none sm:text-sm">{cell.t1_score ?? 0}</span>
                </div>
                <div className="grid grid-cols-[minmax(0,1fr)_10px] items-center gap-x-0.5">
                  <p className="truncate text-[13px] font-semibold leading-tight sm:text-sm">{t2Short}</p>
                  <span className="text-[13px] font-semibold leading-none sm:text-sm">{cell.t2_score ?? 0}</span>
                </div>
              </div>
              <div className="flex items-center justify-between text-[10px] uppercase tracking-[0.16em]">
                <span>{getStatusLabel(cell.status)}</span>
                {movingMatchId === cell.match_id ? <span>Saving</span> : null}
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
          <table className="w-full min-w-[540px] border-separate border-spacing-0 text-xs sm:min-w-[690px] sm:text-sm">
            <thead>
              <tr className="bg-white dark:bg-slate-900">
                <th className="sticky left-0 z-20 w-[76px] border-r border-gray-200 bg-white px-2 py-3 text-left font-medium text-gray-500 dark:border-slate-700 dark:bg-slate-900 dark:text-gray-400 sm:w-[92px]">
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
                    <td className="sticky left-0 z-10 min-w-[76px] border-r border-t border-gray-200 bg-white px-2 py-3 align-middle dark:border-slate-700 dark:bg-slate-900 sm:w-[92px]">
                      <div className="flex min-h-[88px] items-center justify-between gap-2">
                        <div className="space-y-1">
                          <Text variant="primary" className="text-sm font-medium leading-tight text-gray-500 dark:text-gray-400">
                            {getRowTitle(row.label)}
                          </Text>
                          <div className="space-y-0.5 text-sm text-gray-500 dark:text-gray-400">
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
        <ScheduleDetailModal row={selectedCell.row} cell={selectedCell.cell} onClose={() => setSelectedCell(null)} />
      ) : null}

      <Toast message={toastMessage} isOpen={toastOpen} onClose={() => setToastOpen(false)} />
    </div>
  );
}
