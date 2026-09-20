import { useUiStore, type Screen } from "../stores/ui";

const items: { id: Screen; label: string; icon: string }[] = [
  { id: "play", label: "Играть", icon: "▶" },
  { id: "instances", label: "Инстансы", icon: "▦" },
  { id: "accounts", label: "Аккаунты", icon: "☺" },
  { id: "settings", label: "Настройки", icon: "⚙" },
];

export function Sidebar() {
  const { screen, setScreen } = useUiStore();

  return (
    <nav className="flex flex-col w-48 shrink-0" style={{ background: "var(--bg-void)", borderRight: "1px solid var(--bg-stone-2)" }}>
      <div className="flex-1">
        {items.map((item) => (
          <div
            key={item.id}
            className={`nav-item ${screen === item.id ? "nav-item--active" : ""}`}
            role="button"
            tabIndex={0}
            onClick={() => setScreen(item.id)}
            onKeyDown={(e) => e.key === "Enter" && setScreen(item.id)}
          >
            <span aria-hidden="true">{item.icon}</span>
            {item.label}
          </div>
        ))}
      </div>
      <div className="text-center py-2 text-[10px]" style={{ fontFamily: "var(--font-pixel)", color: "var(--text-dim)", letterSpacing: "0.5px" }}>
        сделано командой NetRender
      </div>
    </nav>
  );
}
