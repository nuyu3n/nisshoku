// To-dos as tasks: due dates (Joplin's `todo_due`), how they read, and the
// groups of the task list.

import { api, isoDate, type NoteSummary } from "../api";
import { lang } from "../i18n";
import { report, useStore } from "../store";
import { dialogs } from "../components/Dialogs";
import type { MenuItem } from "../components/ContextMenu";

const L = (ja: string, en: string) => (lang() === "en" ? en : ja);
const pad = (n: number) => String(n).padStart(2, "0");

/** A task due on a day (not at a time) is kept at this hour, for Joplin's alarm. */
const ALL_DAY_HOUR = 9;

export interface Due {
  due: number;
  allDay: boolean;
}

function startOfDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

function dayOffset(due: number, now = new Date()): number {
  return Math.round((startOfDay(new Date(due)).getTime() - startOfDay(now).getTime()) / 86_400_000);
}

/** The time of day `due` falls on, as HH:MM. */
export function dueTime(due: number): string {
  const d = new Date(due);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function isOverdue(task: Due, now = new Date()): boolean {
  if (!task.due) return false;
  return task.allDay ? dayOffset(task.due, now) < 0 : task.due < now.getTime();
}

/** "Today", "Tomorrow 14:30", "Fri", "11/20"… */
export function dueLabel(task: Due, now = new Date()): string {
  const d = new Date(task.due);
  const days = dayOffset(task.due, now);
  const en = lang() === "en";
  let day: string;
  if (days === 0) day = L("今日", "Today");
  else if (days === 1) day = L("明日", "Tomorrow");
  else if (days === -1) day = L("昨日", "Yesterday");
  else if (days > 1 && days < 7) day = d.toLocaleDateString(en ? "en-US" : "ja-JP", { weekday: "short" }) + (en ? "" : "曜");
  else if (d.getFullYear() === now.getFullYear()) day = en ? d.toLocaleDateString("en-US", { month: "short", day: "numeric" }) : `${d.getMonth() + 1}/${d.getDate()}`;
  else day = en ? d.toLocaleDateString("en-US", { year: "numeric", month: "short", day: "numeric" }) : `${d.getFullYear()}/${d.getMonth() + 1}/${d.getDate()}`;
  return task.allDay ? day : `${day} ${dueTime(task.due)}`;
}

function onDay(date: Date, hour = ALL_DAY_HOUR, minute = 0): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate(), hour, minute).getTime();
}

function fromIso(date: string): Date {
  const [y, m, d] = date.split("-").map(Number);
  return new Date(y, m - 1, d);
}

/**
 * The choices for a task's due date. `apply` gets the new due date
 * (`{ due: 0 }` to remove it).
 */
export function dueMenu(current: Due, apply: (due: Due) => void): MenuItem[] {
  const today = new Date();
  const plus = (days: number) => new Date(today.getFullYear(), today.getMonth(), today.getDate() + days);
  const items: MenuItem[] = [
    { label: L("今日", "Today"), icon: "sun", run: () => apply({ due: onDay(today), allDay: true }) },
    { label: L("明日", "Tomorrow"), icon: "calendar", run: () => apply({ due: onDay(plus(1)), allDay: true }) },
    { label: L("来週", "Next week"), icon: "calendar", run: () => apply({ due: onDay(plus(7)), allDay: true }) },
    {
      label: L("日付を選ぶ…", "Pick a date…"),
      icon: "calendar",
      run: async () => {
        const date = await dialogs.date(L("期限の日", "Due date"), isoDate(current.due ? new Date(current.due) : today), { okLabel: L("決定", "Set") });
        if (!date) return;
        // Keeps the time of a timed task.
        const keep = current.due && !current.allDay ? new Date(current.due) : null;
        apply({ due: keep ? onDay(fromIso(date), keep.getHours(), keep.getMinutes()) : onDay(fromIso(date)), allDay: !keep });
      },
    },
    {
      label: current.due && !current.allDay ? L("時刻を変える…", "Change the time…") : L("時刻を指定…", "Set a time…"),
      icon: "clock",
      run: async () => {
        const time = await dialogs.time(L("期限の時刻", "Due time"), current.due && !current.allDay ? dueTime(current.due) : "18:00");
        if (!time) return;
        const [h, m] = time.split(":").map(Number);
        apply({ due: onDay(current.due ? new Date(current.due) : today, h, m), allDay: false });
      },
    },
  ];
  if (current.due) {
    items.push("sep");
    if (!current.allDay) items.push({ label: L("時刻を外す", "Remove the time"), icon: "clock", run: () => apply({ due: onDay(new Date(current.due)), allDay: true }) });
    items.push({ label: L("期限を外す", "Remove the due date"), icon: "x", run: () => apply({ due: 0, allDay: false }) });
  }
  return items;
}

export type TaskGroup = "overdue" | "today" | "upcoming" | "someday" | "done";

export const GROUP_LABELS: Record<TaskGroup, [string, string]> = {
  overdue: ["期限切れ", "Overdue"],
  today: ["今日", "Today"],
  upcoming: ["これから", "Upcoming"],
  someday: ["期限なし", "No due date"],
  done: ["完了", "Done"],
};

/** The tasks by group, each in the order shown. */
export function groupTasks(tasks: NoteSummary[], now = new Date()): Record<TaskGroup, NoteSummary[]> {
  const groups: Record<TaskGroup, NoteSummary[]> = { overdue: [], today: [], upcoming: [], someday: [], done: [] };
  for (const task of tasks) {
    const due = { due: task.todoDue, allDay: task.dueAllDay };
    if (task.todoCompleted) groups.done.push(task);
    else if (!task.todoDue) groups.someday.push(task);
    else if (isOverdue(due, now)) groups.overdue.push(task);
    else if (dayOffset(task.todoDue, now) === 0) groups.today.push(task);
    else groups.upcoming.push(task);
  }
  const byDue = (a: NoteSummary, b: NoteSummary) => a.todoDue - b.todoDue || Number(a.dueAllDay) - Number(b.dueAllDay);
  groups.overdue.sort(byDue);
  // All-day tasks of today first, then by time.
  groups.today.sort((a, b) => Number(b.dueAllDay) - Number(a.dueAllDay) || a.todoDue - b.todoDue);
  groups.upcoming.sort(byDue);
  groups.someday.sort((a, b) => b.order - a.order || b.createdTime - a.createdTime);
  groups.done.sort((a, b) => b.completedTime - a.completedTime);
  return groups;
}

/** Ticks a task off (or back on), with an undo in the toast. */
export async function setTaskDone(task: NoteSummary, done: boolean): Promise<boolean> {
  const s = useStore.getState();
  try {
    await api.setTodoCompleted(task.id, done);
  } catch (e) {
    report(e);
    return false;
  }
  await Promise.all([s.loadNotes(), s.loadFolders()]);
  s.reloadNoteIf(task.id);
  if (done) {
    s.toast(L(`「${task.title || "無題"}」を完了しました`, `Done: ${task.title || "Untitled"}`), {
      label: L("元に戻す", "Undo"),
      run: () => void setTaskDone(task, false),
    });
  }
  return true;
}

export async function setTaskDue(id: string, due: Due) {
  try {
    await api.setTodoDue(id, due.due, due.allDay);
  } catch (e) {
    report(e);
    return;
  }
  const s = useStore.getState();
  await s.loadNotes();
  s.reloadNoteIf(id);
}
