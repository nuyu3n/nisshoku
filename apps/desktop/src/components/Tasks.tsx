// The task list: the to-dos view of the note list. Tasks are added in place,
// grouped by when they are due, and ticked off with a tap.

import { useEffect, useRef, useState } from "react";
import { api, type NoteSummary } from "../api";
import { lang } from "../i18n";
import { report, useStore } from "../store";
import { dueLabel, dueMenu, groupTasks, GROUP_LABELS, isOverdue, setTaskDone, setTaskDue, type Due, type TaskGroup } from "../lib/tasks";
import { folderPath, moveSubmenu, noteMenu } from "../lib/actions";
import { contextMenu, openMenu } from "./ContextMenu";
import { Icon } from "./Icon";
import { isMobile } from "../lib/mobile";

const L = (ja: string, en: string) => (lang() === "en" ? en : ja);

/** Asks the task list to focus its input (the list's + button, the widget). */
export const FOCUS_TASK_INPUT = "nisshoku-focus-task-input";

export function DueChip({ task, onChange, placeholder }: { task: Due; onChange: (due: Due) => void; placeholder?: boolean }) {
  const label = task.due ? dueLabel(task) : placeholder ? L("期限", "Due") : null;
  return (
    <button
      type="button"
      className={`due-chip${task.due ? "" : " unset"}${isOverdue(task) ? " overdue" : ""}`}
      title={L("期限", "Due date")}
      onClick={(e) => {
        e.stopPropagation();
        openMenu(e.currentTarget, dueMenu(task, onChange));
      }}
    >
      <Icon name="calendar" />
      {label}
    </button>
  );
}

function TaskRow({ task, folder }: { task: NoteSummary; folder?: string }) {
  const selectedId = useStore((s) => s.selectedId);
  // Ticked rows linger a moment before they move to "Done".
  const [ticked, setTicked] = useState(false);
  const done = task.todoCompleted || ticked;
  return (
    <div className={`task-row${done ? " done" : ""}${ticked ? " ticked" : ""}${task.id === selectedId ? " active" : ""}`} {...contextMenu(() => noteMenu(task))}>
      <button
        className="task-check"
        role="checkbox"
        aria-checked={done}
        title={done ? L("未完了に戻す", "Mark as not done") : L("完了にする", "Mark as done")}
        onClick={(e) => {
          if (task.todoCompleted) return void setTaskDone(task, false);
          if (ticked) return; // already on its way
          setTicked(true);
          // Its height, for folding away when it leaves the open tasks (lib/motion.ts).
          const row = e.currentTarget.closest<HTMLElement>(".task-row");
          if (row) row.dataset.h = String(row.offsetHeight);
          // Not saved: show it open again.
          window.setTimeout(() => void setTaskDone(task, true).then((ok) => ok || setTicked(false)), 350);
        }}
      >
        <Icon name="check" />
      </button>
      <button className="task-main" onClick={() => useStore.getState().select(task.id)}>
        <span className={`task-title${task.title ? "" : " untitled"}`}>{task.title || L("無題", "Untitled")}</span>
        {(folder || task.excerpt) && <span className="task-sub">{[folder, task.excerpt].filter(Boolean).join(" · ")}</span>}
      </button>
      {!task.todoCompleted && <DueChip task={{ due: task.todoDue, allDay: task.dueAllDay }} onChange={(due) => void setTaskDue(task.id, due)} />}
    </div>
  );
}

export function TaskList({ tasks }: { tasks: NoteSummary[] }) {
  const scope = useStore((s) => s.scope);
  const folders = useStore((s) => s.folders);
  const [title, setTitle] = useState("");
  const [due, setDue] = useState<Due>({ due: 0, allDay: false });
  const [showDone, setShowDone] = useState(false);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const focus = () => input.current?.focus();
    window.addEventListener(FOCUS_TASK_INPUT, focus);
    return () => window.removeEventListener(FOCUS_TASK_INPUT, focus);
  }, []);

  const taskNotebook = useStore((s) => s.prefs.taskNotebook);
  // In a notebook's list, tasks go there; elsewhere to the chosen notebook
  // (remembered), if it still exists, or the default one.
  const inFolder = scope.kind === "folder" ? scope.id : null;
  const chosen = folders.some((f) => f.id === taskNotebook) ? taskNotebook : "";
  const parent = inFolder ?? (chosen || null);

  const chooseNotebook = (anchor: HTMLElement) =>
    openMenu(anchor, [
      { label: L("既定のノートブック", "Default notebook"), icon: "folder", checked: !chosen, run: () => useStore.getState().setPrefs({ taskNotebook: "" }) },
      "sep",
      ...moveSubmenu(
        (f) => f.isDiary,
        (id) => useStore.getState().setPrefs({ taskNotebook: id }),
      ),
    ]);

  const add = async () => {
    if (!title.trim()) return;
    const created = await api.createTask(title.trim(), parent, due.due, due.allDay).catch(report);
    if (!created) return;
    setTitle("");
    setDue({ due: 0, allDay: false });
    const s = useStore.getState();
    await Promise.all([s.loadNotes(), s.loadFolders()]);
    input.current?.focus();
  };

  const groups = groupTasks(tasks);
  const folderOf = (t: NoteSummary) => (scope.kind === "folder" ? undefined : folders.find((f) => f.id === t.parentId)?.title);
  const open = tasks.length - groups.done.length;
  const order: TaskGroup[] = ["overdue", "today", "upcoming", "someday"];

  return (
    <div className="task-list">
      <form
        className="task-add"
        onSubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <Icon name="plus" />
        <input
          ref={input}
          className="task-input"
          placeholder={L("タスクを追加", "Add a task")}
          value={title}
          enterKeyHint="done"
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => {
            // Enter while composing (Japanese input) only confirms the text. Phone
            // keyboards send Enter while still composing, so there only the button adds
            // (as in AddInput).
            if (e.key === "Enter" && (isMobile || e.nativeEvent.isComposing)) e.preventDefault();
          }}
        />
        <DueChip task={due} onChange={setDue} placeholder />
        {!inFolder && (
          <button
            type="button"
            className={`due-chip folder-chip${chosen ? "" : " unset"}`}
            title={L("追加先のノートブック", "Notebook to add to")}
            onClick={(e) => chooseNotebook(e.currentTarget)}
          >
            <Icon name="folder" />
            <span className="folder-chip-name">{chosen ? folderPath(chosen).join(" / ") : L("既定", "Default")}</span>
          </button>
        )}
        {(title.trim() || isMobile) && (
          <button className="btn primary task-add-btn" type="submit" disabled={!title.trim()}>
            {L("追加", "Add")}
          </button>
        )}
      </form>

      {open === 0 && (
        <div className="empty">
          {groups.done.length ? L("すべて完了しました 🎉", "All done 🎉") : L("タスクはまだありません。上の欄に書いて Enter で追加できます。", "No tasks yet. Type one above and press Enter.")}
        </div>
      )}

      {order.map(
        (g) =>
          groups[g].length > 0 && (
            <section key={g} className={`task-group ${g}`}>
              <h4>
                {L(...GROUP_LABELS[g])}
                <span className="count">{groups[g].length}</span>
              </h4>
              {groups[g].map((t) => (
                <TaskRow key={t.id} task={t} folder={folderOf(t)} />
              ))}
            </section>
          ),
      )}

      {groups.done.length > 0 && (
        <section className="task-group done">
          <button className="task-group-toggle" onClick={() => setShowDone(!showDone)}>
            <Icon name={showDone ? "chevronDown" : "chevronRight"} />
            {L(...GROUP_LABELS.done)}
            <span className="count">{groups.done.length}</span>
          </button>
          {showDone && groups.done.slice(0, 100).map((t) => <TaskRow key={t.id} task={t} folder={folderOf(t)} />)}
        </section>
      )}
    </div>
  );
}
