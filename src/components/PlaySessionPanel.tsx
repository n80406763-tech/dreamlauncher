import { useEffect, useRef } from "react";
import { ProgressBar } from "./ui/ProgressBar";
import type { PlaySession } from "../stores/play";

export function PlaySessionPanel({ session }: { session: PlaySession }) {
  const logRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [session.log.length]);

  if (session.phase === "idle") return null;

  const statusColor = session.phase === "error" ? "var(--danger-hi)" : session.phase === "exited" && session.exitCode !== 0 ? "var(--warn)" : "var(--text-dim)";

  return (
    <div className="flex flex-col gap-2">
      <div className="text-xs" style={{ color: statusColor }}>
        {session.message}
      </div>
      {session.phase === "installing" && session.progress ? (
        <ProgressBar value={session.progress.total > 0 ? session.progress.done / session.progress.total : 0} label={`${session.progress.done} / ${session.progress.total} файлов`} />
      ) : null}
      {session.log.length > 0 ? (
        <div
          ref={logRef}
          data-selectable
          className="p-2 text-xs overflow-y-auto"
          style={{ background: "var(--bg-slot)", color: "var(--text)", fontFamily: "var(--font-mono)", maxHeight: 140 }}
        >
          {session.log.map((line, i) => (
            <div key={i} className="whitespace-pre-wrap break-all">
              {line}
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
}
