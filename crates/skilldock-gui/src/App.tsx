import { useCallback, useEffect, useState, type ReactNode } from "react";
import { RefreshCw, Stethoscope, DownloadCloud, ArrowUpCircle } from "lucide-react";
import { commands, type ConsumerArg, type Listing, type Report } from "./bindings";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: string };

export default function App() {
  const [state, setState] = useState<Listing | null>(null);
  const [report, setReport] = useState<Report | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [consumer, setConsumer] = useState("");
  const [repo, setRepo] = useState("");
  const [addSkills, setAddSkills] = useState("");

  /** Run a command, refresh the dashboard, and surface a status line. */
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
      const s = await commands.getState();
      if (s.status === "ok") setState(s.data);
      setStatus(`${label}: done`);
      setBusy(false);
      return r.data;
    },
    [],
  );

  const load = useCallback(async () => {
    const s = await commands.getState();
    if (s.status === "ok") setState(s.data);
    else setStatus(`load: ${s.error}`);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const consumerArg = (): ConsumerArg =>
    consumer.trim() ? { kind: "project", path: consumer.trim() } : { kind: "global" };

  return (
    <div className="mx-auto min-h-full max-w-3xl bg-zinc-50 px-6 py-8 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
      <header className="mb-5 flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">skilldock</h1>
          <p className="text-sm text-zinc-500 dark:text-zinc-400">Your Agent Skills, by provenance.</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => void load()} disabled={busy}>
            <RefreshCw className="size-4" /> Refresh
          </Button>
          <Button variant="outline" size="sm" onClick={() => void run("sync", commands.sync())} disabled={busy}>
            <DownloadCloud className="size-4" /> Sync
          </Button>
          <Button variant="outline" size="sm" onClick={() => void run("update", commands.update([]))} disabled={busy}>
            <ArrowUpCircle className="size-4" /> Update all
          </Button>
          <Button
            size="sm"
            onClick={() =>
              void run("doctor", commands.doctor(false, false, true)).then((r) => r && setReport(r))
            }
            disabled={busy}
          >
            <Stethoscope className="size-4" /> Doctor
          </Button>
        </div>
      </header>

      {status && (
        <p className="mb-4 rounded-md bg-zinc-100 px-3 py-2 text-sm text-zinc-700 dark:bg-zinc-900 dark:text-zinc-300">
          {status}
        </p>
      )}

      <Card className="mb-6">
        <CardHeader>
          <CardTitle>Add a vendored skill</CardTitle>
          <CardDescription>Declare a source repo and the skill path(s) or glob(s) to vendor.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap items-center gap-2">
          <Input placeholder="owner/repo or git URL" value={repo} onChange={setRepo} className="flex-1" />
          <Input placeholder="skills/a  skills/b/*" value={addSkills} onChange={setAddSkills} className="flex-1" />
          <Button
            size="sm"
            disabled={busy || !repo.trim() || !addSkills.trim()}
            onClick={() => {
              const skills = addSkills.split(/\s+/).filter(Boolean);
              void run("add", commands.add(repo.trim(), skills, null)).then((ok) => {
                if (ok) {
                  setRepo("");
                  setAddSkills("");
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
          <CardTitle>Consumer</CardTitle>
          <CardDescription>
            A project path to link into. Leave blank to target the global config (`~/.agents` + `~/.claude`).
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Input placeholder="/path/to/project (blank = global)" value={consumer} onChange={setConsumer} />
        </CardContent>
      </Card>

      {state && (
        <div className="flex flex-col gap-6">
          <SkillSection title="Authored" count={state.authored.length} accent="authored">
            {state.authored.length === 0 && <Empty>No authored skills yet.</Empty>}
            {state.authored.map((s) => (
              <Row key={s.name} name={s.name} badge="authored">
                {!s.present && <span className="text-xs text-red-600 dark:text-red-400">missing</span>}
                <LinkButtons busy={busy} onLink={() => void run(`link ${s.name}`, commands.link(consumerArg(), [s.name], false))} onUnlink={() => void run(`unlink ${s.name}`, commands.unlink(consumerArg(), [s.name]))} />
              </Row>
            ))}
          </SkillSection>

          <SkillSection title="Vendored" count={state.vendored.length} accent="vendored">
            {state.vendored.length === 0 && <Empty>No vendored skills yet.</Empty>}
            {state.vendored.map((s) => (
              <Row key={`${s.repo}#${s.name}`} name={s.name} badge="vendored">
                <span className="truncate text-xs text-zinc-500 dark:text-zinc-400">{s.repo}</span>
                <span className="font-mono text-xs text-zinc-400">{s.resolved.slice(0, 12)}</span>
                <LinkButtons busy={busy} onLink={() => void run(`link ${s.name}`, commands.link(consumerArg(), [s.name], false))} onUnlink={() => void run(`unlink ${s.name}`, commands.unlink(consumerArg(), [s.name]))} />
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

function LinkButtons({ busy, onLink, onUnlink }: { busy: boolean; onLink: () => void; onUnlink: () => void }) {
  return (
    <div className="flex gap-1">
      <Button variant="outline" size="sm" disabled={busy} onClick={onLink}>
        Link
      </Button>
      <Button variant="outline" size="sm" disabled={busy} onClick={onUnlink}>
        Unlink
      </Button>
    </div>
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
