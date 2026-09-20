import { Button } from "../components/ui/Button";
import { Panel } from "../components/ui/Panel";
import { useAccounts, useInstances } from "../hooks/queries";
import { useUiStore } from "../stores/ui";
import { useEffect, useState } from "react";

export function PlayScreen() {
  const { data: instances } = useInstances();
  const { data: accounts } = useAccounts();
  const activeAccount = accounts?.find((a) => a.is_active);
  const setScreen = useUiStore((s) => s.setScreen);
  const [stars, setStars] = useState<{ id: number; left: string; top: string; delay: string; duration: string; size: string }[]>([]);

  useEffect(() => {
    // Generate random stars for the background
    const newStars = Array.from({ length: 50 }).map((_, i) => ({
      id: i,
      left: `${Math.random() * 100}%`,
      top: `${Math.random() * 100}%`,
      delay: `${Math.random() * 5}s`,
      duration: `${3 + Math.random() * 4}s`,
      size: `${Math.random() * 3 + 1}px`
    }));
    setStars(newStars);
  }, []);

  return (
    <div className="relative flex flex-col items-center justify-center h-full gap-6 p-6 overflow-hidden" style={{ background: "radial-gradient(circle at 50% 20%, #0a0e1c 0%, #020306 80%)" }}>
      {/* Starry background */}
      {stars.map((star) => (
        <div
          key={star.id}
          className="absolute rounded-full bg-white opacity-0"
          style={{
            left: star.left,
            top: star.top,
            width: star.size,
            height: star.size,
            animation: `twinkle ${star.duration} infinite ${star.delay} linear`,
            boxShadow: `0 0 ${parseFloat(star.size) * 2}px rgba(255, 255, 255, 0.8)`
          }}
        />
      ))}

      {/* Nebula effect overlay */}
      <div 
        className="absolute inset-0 pointer-events-none opacity-30" 
        style={{ 
          background: "radial-gradient(ellipse at 80% 30%, rgba(107, 60, 226, 0.3) 0%, transparent 50%), radial-gradient(ellipse at 20% 80%, rgba(0, 210, 255, 0.2) 0%, transparent 50%)",
          mixBlendMode: "screen"
        }} 
      />

      <div className="text-center relative z-10">
        <h1 className="font-pixel text-4xl mb-3 drop-shadow-[0_0_15px_rgba(107,60,226,0.8)]" style={{ fontFamily: "var(--font-pixel)", color: "#fff" }}>
          DreamLauncher
        </h1>
        <p className="tracking-widest uppercase text-xs" style={{ color: "var(--info)" }}>
          SPACE FLIGHT EDITION
        </p>
        <p className="mt-2 text-xs" style={{ color: "var(--text-dim)" }}>
          Не является продуктом Mojang или Microsoft.
        </p>
      </div>

      <Panel className="w-full max-w-sm flex flex-col gap-4 relative z-10" style={{ 
        background: "rgba(10, 12, 26, 0.7)", 
        backdropFilter: "blur(8px)",
        border: "1px solid rgba(107, 60, 226, 0.3)",
        boxShadow: "0 0 20px rgba(107, 60, 226, 0.15), inset 0 0 10px rgba(0, 210, 255, 0.1)"
      }}>
        <div className="flex items-center justify-between text-sm">
          <span style={{ color: "var(--text-dim)" }}>Пилот</span>
          {activeAccount ? (
            <span className="font-bold flex items-center gap-2" style={{ color: "var(--text-hi)", textShadow: "0 0 5px var(--accent)" }}>
              <div className="w-2 h-2 rounded-full bg-green-400 animate-pulse" />
              {activeAccount.username}
            </span>
          ) : (
            <Button size="sm" onClick={() => setScreen("accounts")}>
              Авторизация
            </Button>
          )}
        </div>
        <div className="flex items-center justify-between text-sm">
          <span style={{ color: "var(--text-dim)" }}>Корабли (Инстансы)</span>
          <span className="font-bold" style={{ color: "var(--info)" }}>{instances?.length ?? 0}</span>
        </div>
        <Button
          variant="primary"
          disabled={!activeAccount || !instances?.length}
          onClick={() => setScreen("instances")}
          className="mt-2 relative overflow-hidden group"
          style={{
            background: "linear-gradient(90deg, var(--accent) 0%, var(--accent-hi) 100%)",
            border: "none",
            boxShadow: "0 0 15px var(--accent)"
          }}
        >
          {instances?.length ? "ЗАПУСК ДВИГАТЕЛЕЙ" : "ПОДГОТОВИТЬ КОРАБЛЬ"}
        </Button>
      </Panel>

      <style>{`
        @keyframes twinkle {
          0% { opacity: 0; transform: scale(0.5); }
          50% { opacity: 1; transform: scale(1.2); }
          100% { opacity: 0; transform: scale(0.5); }
        }
      `}</style>
    </div>
  );
}
