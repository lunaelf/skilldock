import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { RefreshCw, Stethoscope, DownloadCloud, ArrowUpCircle, FolderOpen } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { commands, type ConsumerArg, type LinkState, type Listing, type Report } from "./bindings";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: string };

const GLOBAL: ConsumerArg = { kind: "global" };

/** Last path segment, for a compact Consumer label. */
const basename = (p: string) => p.replace(/\/+$/, "").split("/").pop() || p;

const consumerLabel = (c: ConsumerArg) => (c.kind === "global" ? "Global" : basename(c.path));

export default function App() {
  const [state, setState] = useState<Listing | null>(null);
  const [report, setReport] = useState<Report | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Registry (Consumers panel).
  const [consumers, setConsumers] = useState<string[]>([]);
  const [selected, setSelected] = useState<ConsumerArg>(GLOBAL);
  const [linkState, setLinkState] = useState<Record<string, LinkState>>({});
  const [projectPath, setProjectPath] = useState("");

  // Add card.
  const [repo, setRepo] = useState("");
  const [addSkills, setAddSkills] = useState("");
  const [addRef, setAddRef] = useState("");

  // Author card.
  const [authorName, setAuthorName] = useState("");

  // Doctor toggles (read-only by default).
  const [verify, setVerify] = useState(false);
  const [fix, setFix] = useState(false);

  // Pending `link --force` confirmation (a link points elsewhere).
  const [pendingForce, setPendingForce] = useState<string | null>(null);

  // The selected Consumer, readable from memoized callbacks without staleness.
  const selectedRef = useRef(selected);
  useEffect(() => {
    selectedRef.current = selected;
  }, [selected]);

  const loadState = useCallback(async () => {
    const s = await commands.getState();
    if (s.status === "ok") setState(s.data);
    else setStatus(`load: ${s.error}`);
  }, []);

  const loadConsumers = useCallback(async () => {
    const c = await commands.registeredConsumers();
    if (c.status === "ok") setConsumers(c.data);
    else setStatus(`consumers: ${c.error}`);
  }, []);

  const loadLinkState = useCallback(async (consumer: ConsumerArg) => {
    const s = await commands.linkStatus(consumer);
    if (s.status !== "ok") {
      setLinkState({});
      return;
    }
    const map: Record<string, LinkState> = {};
    for (const it of s.data) map[it.name] = it.state;
    setLinkState(map);
  }, []);

  const refreshAll = useCallback(async () => {
    await loadState();
    await loadConsumers();
    await loadLinkState(selectedRef.current);
  }, [loadState, loadConsumers, loadLinkState]);

  useEffect(() => {
    void loadState();
    void loadConsumers();
  }, [loadState, loadConsumers]);

  // Re-read link state whenever the active Consumer changes.
  useEffect(() => {
    void loadLinkState(selected);
  }, [selected, loadLinkState]);

  // If the selected project drops out of the Registry — deregistered directly,
  // or auto-deregistered when its last link is unlinked/pruned — fall back to
  // Global so "Acting on …" never names a Consumer that's gone.
  useEffect(() => {
    if (selected.kind === "project" && !consumers.includes(selected.path)) {
      setSelected(GLOBAL);
    }
  }, [consumers, selected]);

  /** Run a command, refresh everything, and surface a status line. */
  const run = useCallback(
    async <T,>(label: string, p: Promise<Result<T>>): Promise<T | null> => {
      setBusy(true);
      setStatus(`${label}…`);
      const r = await p;
      if (r.status === "error") {
        setStatus(`${label}: ${r.error}`);
        setBusy(false);
        return null;
      }
      await refreshAll();
      setStatus(`${label}: done`);
      setBusy(false);
      return r.data;
    },
    [refreshAll],
  );

  /** Link, but on a conflicting existing link, offer to replace with `--force`. */
  const doLink = useCallback(
    async (skill: string) => {
      setBusy(true);
      setStatus(`link ${skill}…`);
      const r = await commands.link(selectedRef.current, [skill], false);
      if (r.status === "ok") {
        await refreshAll();
        setStatus(`link ${skill}: done`);
      } else if (r.error.includes("--force")) {
        setPendingForce(skill);
        setStatus(`link ${skill}: already linked elsewhere`);
      } else {
        setStatus(`link ${skill}: ${r.error}`);
      }
      setBusy(false);
    },
    [refreshAll],
  );

  const confirmForce = useCallback(async () => {
    if (!pendingForce) return;
    const skill = pendingForce;
    setPendingForce(null);
    await run(`replace ${skill}`, commands.link(selectedRef.current, [skill], true));
  }, [pendingForce, run]);

  /** Register a project (from the folder picker or the paste box). */
  const addProject = useCallback((path: string) => run("add project", commands.register(path)), [run]);

  /** Register a project chosen from the native folder picker. */
  const browseProject = useCallback(async () => {
    const dir = await open({ directory: true, multiple: false, title: "Add a project" });
    if (typeof dir === "string") await addProject(dir);
  }, [addProject]);

  const deregister = useCallback((path: string) => run("deregister", commands.deregister(path)), [run]);

  /** Reveal an authored skill's SKILL.md in the OS file manager. */
  const reveal = useCallback(async (name: string) => {
    const r = await commands.authoredSkillDir(name);
    if (r.status !== "ok") {
      setStatus(`reveal: ${r.error}`);
      return;
    }
    try {
      await revealItemInDir(`${r.data}/SKILL.md`);
    } catch (e) {
      setStatus(`reveal: ${String(e)}`);
    }
  }, []);

  return (
    <div className="mx-auto min-h-full max-w-3xl bg-zinc-50 px-6 py-8 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
      <header className="mb-5 flex items-start justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">skilldock</h1>
          <p className="text-sm text-zinc-500 dark:text-zinc-400">Your Agent Skills, by provenance.</p>
        </div>
        <div className="flex flex-col items-end gap-2">
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={() => void refreshAll()} disabled={busy}>
              <RefreshCw className="size-4" /> Refresh
            </Button>
            <Button variant="outline" size="sm" onClick={() => void run("sync", commands.sync())} disabled={busy}>
              <DownloadCloud className="size-4" /> Sync
            </Button>
            <Button variant="outline" size="sm" onClick={() => void run("update", commands.update([]))} disabled={busy}>
              <ArrowUpCircle className="size-4" /> Update all
            </Button>
          </div>
          <div className="flex items-center gap-3">
            <Check label="verify" checked={verify} onChange={setVerify} />
            <Check label="fix" checked={fix} onChange={setFix} />
            <Button
              size="sm"
              onClick={() => void run("doctor", commands.doctor(verify, fix, true)).then((r) => r && setReport(r))}
              disabled={busy}
            >
              <Stethoscope className="size-4" /> Doctor
            </Button>
          </div>
        </div>
      </header>

      {status && (
        <p className="mb-4 rounded-md bg-zinc-100 px-3 py-2 text-sm text-zinc-700 dark:bg-zinc-900 dark:text-zinc-300">
          {status}
        </p>
      )}

      {pendingForce && (
        <div className="mb-4 flex items-center gap-3 rounded-md border border-amber-300 bg-amber-50 px-3 py-2 text-sm dark:border-amber-800/60 dark:bg-amber-950/40">
          <span>
            <span className="font-semibold">{pendingForce}</span> already links elsewhere in{" "}
            {consumerLabel(selected)}. Replace it?
          </span>
          <div className="ml-auto flex gap-2">
            <Button size="sm" disabled={busy} onClick={() => void confirmForce()}>
              Replace
            </Button>
            <Button variant="outline" size="sm" disabled={busy} onClick={() => setPendingForce(null)}>
              Cancel
            </Button>
          </div>
        </div>
      )}

      <Card className="mb-6">
        <CardHeader>
          <CardTitle>Add a vendored skill</CardTitle>
          <CardDescription>
            Declare a source repo and the skill path(s) or glob(s) to vendor. Leave the ref blank to pin the repo's
            default branch.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap items-center gap-2">
          <Input placeholder="owner/repo or git URL" value={repo} onChange={setRepo} className="flex-1" />
          <Input placeholder="skills/a  skills/b/*" value={addSkills} onChange={setAddSkills} className="flex-1" />
          <Input placeholder="ref (branch/tag, optional)" value={addRef} onChange={setAddRef} className="w-44" />
          <Button
            size="sm"
            disabled={busy || !repo.trim() || !addSkills.trim()}
            onClick={() => {
              const skills = addSkills.split(/\s+/).filter(Boolean);
              void run("add", commands.add(repo.trim(), skills, addRef.trim() || null)).then((ok) => {
                if (ok) {
                  setRepo("");
                  setAddSkills("");
                  setAddRef("");
                }
              });
            }}
          >
            Add
          </Button>
        </CardContent>
      </Card>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle>Author a skill</CardTitle>
          <CardDescription>
            Scaffold a new authored skill in the Store and record it. Edit its SKILL.md in your own editor via Reveal.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap items-center gap-2">
          <Input placeholder="skill-name" value={authorName} onChange={setAuthorName} className="flex-1" />
          <Button
            size="sm"
            disabled={busy || !authorName.trim()}
            onClick={() =>
              void run("author", commands.author(authorName.trim())).then((ok) => {
                if (ok) setAuthorName("");
              })
            }
          >
            Create
          </Button>
        </CardContent>
      </Card>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle>Consumers</CardTitle>
          <CardDescription>
            Pick where to link. Global targets the global config (`~/.agents` + `~/.claude`); projects come from the
            Registry.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="flex flex-wrap gap-2">
            <ConsumerChip
              label="Global"
              active={selected.kind === "global"}
              onSelect={() => setSelected(GLOBAL)}
            />
            {consumers.map((path) => (
              <ConsumerChip
                key={path}
                label={basename(path)}
                title={path}
                active={selected.kind === "project" && selected.path === path}
                busy={busy}
                onSelect={() => setSelected({ kind: "project", path })}
                onRemove={() => void deregister(path)}
              />
            ))}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Button variant="outline" size="sm" disabled={busy} onClick={() => void browseProject()}>
              <FolderOpen className="size-4" /> Browse…
            </Button>
            <Input
              placeholder="/path/to/project (paste to add)"
              value={projectPath}
              onChange={setProjectPath}
              className="flex-1"
            />
            <Button
              variant="outline"
              size="sm"
              disabled={busy || !projectPath.trim()}
              onClick={() =>
                void addProject(projectPath.trim()).then((ok) => {
                  if (ok !== null) setProjectPath("");
                })
              }
            >
              Add project
            </Button>
          </div>

          <div className="flex items-center gap-2 text-sm text-zinc-500 dark:text-zinc-400">
            <span>
              Acting on <span className="font-medium text-zinc-700 dark:text-zinc-200">{consumerLabel(selected)}</span>
            </span>
            <div className="ml-auto flex gap-2">
              <Button variant="outline" size="sm" disabled={busy} onClick={() => void run("prune", commands.prune(selected))}>
                Prune
              </Button>
              <Button variant="outline" size="sm" disabled={busy} onClick={() => void run("relink", commands.relink(selected))}>
                Relink
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {state && (
        <div className="flex flex-col gap-6">
          <SkillSection title="Authored" count={state.authored.length} accent="authored">
            {state.authored.length === 0 && <Empty>No authored skills yet.</Empty>}
            {state.authored.map((s) => (
              <Row key={s.name} name={s.name} badge="authored">
                {!s.present && <span className="text-xs text-red-600 dark:text-red-400">missing</span>}
                <LinkControl
                  state={linkState[s.name]}
                  busy={busy}
                  onLink={() => void doLink(s.name)}
                  onUnlink={() => void run(`unlink ${s.name}`, commands.unlink(selected, [s.name]))}
                />
                <Button
                  variant="outline"
                  size="sm"
                  disabled={busy || !s.present}
                  title={s.present ? "Reveal SKILL.md in the file manager" : "No SKILL.md in the Store"}
                  onClick={() => void reveal(s.name)}
                >
                  Reveal
                </Button>
              </Row>
            ))}
          </SkillSection>

          <SkillSection title="Vendored" count={state.vendored.length} accent="vendored">
            {state.vendored.length === 0 && <Empty>No vendored skills yet.</Empty>}
            {state.vendored.map((s) => (
              <Row key={`${s.repo}#${s.name}`} name={s.name} badge="vendored">
                <span className="truncate text-xs text-zinc-500 dark:text-zinc-400">{s.repo}</span>
                <span className="font-mono text-xs text-zinc-400">{s.resolved.slice(0, 12)}</span>
                <LinkControl
                  state={linkState[s.name]}
                  busy={busy}
                  onLink={() => void doLink(s.name)}
                  onUnlink={() => void run(`unlink ${s.name}`, commands.unlink(selected, [s.name]))}
                />
                <Button
                  variant="outline"
                  size="sm"
                  disabled={busy}
                  onClick={() => void run(`update ${s.repo}`, commands.update([s.repo]))}
                >
                  Update
                </Button>
                <Button variant="outline" size="sm" disabled={busy} onClick={() => void run(`remove ${s.name}`, commands.remove(s.name))}>
                  Remove
                </Button>
              </Row>
            ))}
          </SkillSection>
        </div>
      )}

      {report && <DoctorPanel report={report} />}
    </div>
  );
}

/** The per-skill Link/Unlink toggle: reflects the skill's state in the selected Consumer. */
function LinkControl({
  state,
  busy,
  onLink,
  onUnlink,
}: {
  state?: LinkState;
  busy: boolean;
  onLink: () => void;
  onUnlink: () => void;
}) {
  const st = state ?? "unlinked";
  if (st === "unlinked") {
    return (
      <Button variant="outline" size="sm" disabled={busy} onClick={onLink}>
        Link
      </Button>
    );
  }
  return (
    <div className="flex items-center gap-2">
      {st === "dangling" ? (
        <span className="text-xs font-medium text-red-600 dark:text-red-400">dangling</span>
      ) : (
        <span className="text-xs text-emerald-600 dark:text-emerald-400">linked</span>
      )}
      <Button variant="outline" size="sm" disabled={busy} onClick={onUnlink}>
        Unlink
      </Button>
    </div>
  );
}

/** A selectable Consumer in the Registry list; projects carry a deregister ×. */
function ConsumerChip({
  label,
  title,
  active,
  busy,
  onSelect,
  onRemove,
}: {
  label: string;
  title?: string;
  active: boolean;
  busy?: boolean;
  onSelect: () => void;
  onRemove?: () => void;
}) {
  return (
    <span
      className={cn(
        "inline-flex h-8 items-center gap-1 rounded-md border px-2 text-xs",
        active
          ? "border-zinc-900 bg-zinc-900 text-zinc-50 dark:border-zinc-100 dark:bg-zinc-100 dark:text-zinc-900"
          : "border-zinc-200 dark:border-zinc-800",
      )}
    >
      <button type="button" title={title} onClick={onSelect} className="font-medium">
        {label}
      </button>
      {onRemove && (
        <button
          type="button"
          title="Deregister"
          onClick={onRemove}
          disabled={busy}
          className="opacity-60 hover:opacity-100 disabled:pointer-events-none"
        >
          ×
        </button>
      )}
    </span>
  );
}

function DoctorPanel({ report }: { report: Report }) {
  const errors = report.findings.filter((f) => f.severity === "error").length;
  const warnings = report.findings.length - errors;
  return (
    <Card className="mt-6">
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle>Doctor</CardTitle>
        <div className="flex gap-2">
          <Badge variant={errors > 0 ? "vendored" : "muted"}>{errors} errors</Badge>
          <Badge variant="muted">{warnings} warnings</Badge>
        </div>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {report.findings.length === 0 && <Empty>Everything checks out.</Empty>}
        {report.findings.map((f, i) => (
          <div key={i} className="flex items-center gap-2 text-sm">
            <Badge variant={f.severity === "error" ? "vendored" : "muted"}>{f.kind}</Badge>
            <span className="font-medium">{f.subject}</span>
            <span className="ml-auto truncate text-xs text-zinc-500 dark:text-zinc-400">{f.detail}</span>
          </div>
        ))}
      </CardContent>
    </Card>
  );
}

function Check({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="flex select-none items-center gap-1 text-xs text-zinc-600 dark:text-zinc-400">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="size-3.5 accent-zinc-900 dark:accent-zinc-100"
      />
      {label}
    </label>
  );
}

function Input({
  value,
  onChange,
  placeholder,
  className,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  className?: string;
}) {
  return (
    <input
      className={
        "h-9 rounded-md border border-zinc-200 bg-transparent px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-zinc-400 dark:border-zinc-800 " +
        (className ?? "")
      }
      placeholder={placeholder}
      value={value}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

function SkillSection({
  title,
  count,
  accent,
  children,
}: {
  title: string;
  count: number;
  accent: "authored" | "vendored";
  children: ReactNode;
}) {
  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle>{title}</CardTitle>
        <Badge variant={accent}>{count}</Badge>
      </CardHeader>
      <CardContent className="flex flex-col divide-y divide-zinc-100 dark:divide-zinc-800">
        {children}
      </CardContent>
    </Card>
  );
}

function Row({
  name,
  badge,
  children,
}: {
  name: string;
  badge: "authored" | "vendored";
  children?: ReactNode;
}) {
  return (
    <div className="flex items-center gap-3 py-2 first:pt-0 last:pb-0">
      <span className="font-medium">{name}</span>
      <Badge variant={badge}>{badge}</Badge>
      <div className="ml-auto flex items-center gap-3 overflow-hidden">{children}</div>
    </div>
  );
}

function Empty({ children }: { children: ReactNode }) {
  return <p className="py-2 text-sm text-zinc-500 dark:text-zinc-400">{children}</p>;
}
