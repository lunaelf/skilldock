import { useCallback, useEffect, useState, type ReactNode } from "react";
import { RefreshCw } from "lucide-react";
import { commands, type Listing } from "./bindings";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

export default function App() {
  const [state, setState] = useState<Listing | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    const res = await commands.getState();
    if (res.status === "ok") {
      setState(res.data);
      setError(null);
    } else {
      setError(res.error);
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="mx-auto min-h-full max-w-3xl bg-zinc-50 px-6 py-8 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
      <header className="mb-6 flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">skilldock</h1>
          <p className="text-sm text-zinc-500 dark:text-zinc-400">Your Agent Skills, by provenance.</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
          <RefreshCw className={loading ? "size-4 animate-spin" : "size-4"} />
          Refresh
        </Button>
      </header>

      {error && (
        <Card className="mb-6 border-red-300 dark:border-red-900">
          <CardHeader>
            <CardTitle className="text-red-700 dark:text-red-400">Couldn’t read the dock</CardTitle>
            <CardDescription>{error}</CardDescription>
          </CardHeader>
        </Card>
      )}

      {state && (
        <div className="flex flex-col gap-6">
          <SkillSection title="Authored" count={state.authored.length} accent="authored">
            {state.authored.length === 0 && <Empty>No authored skills yet.</Empty>}
            {state.authored.map((s) => (
              <Row key={s.name} name={s.name} badge="authored">
                {!s.present && <span className="text-xs text-red-600 dark:text-red-400">missing</span>}
              </Row>
            ))}
          </SkillSection>

          <SkillSection title="Vendored" count={state.vendored.length} accent="vendored">
            {state.vendored.length === 0 && <Empty>No vendored skills yet.</Empty>}
            {state.vendored.map((s) => (
              <Row key={`${s.repo}#${s.name}`} name={s.name} badge="vendored">
                <span className="truncate text-xs text-zinc-500 dark:text-zinc-400">{s.repo}</span>
                <span className="font-mono text-xs text-zinc-400">{s.resolved.slice(0, 12)}</span>
              </Row>
            ))}
          </SkillSection>
        </div>
      )}
    </div>
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
