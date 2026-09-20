import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { ToastContainer } from "./components/ui/Toast";
import { AccountsScreen } from "./screens/AccountsScreen";
import { InstancesScreen } from "./screens/InstancesScreen";
import { PlayScreen } from "./screens/PlayScreen";
import { SettingsScreen } from "./screens/SettingsScreen";
import { useUiStore } from "./stores/ui";

function ActiveScreen() {
  const screen = useUiStore((s) => s.screen);
  switch (screen) {
    case "play":
      return <PlayScreen />;
    case "instances":
      return <InstancesScreen />;
    case "accounts":
      return <AccountsScreen />;
    case "settings":
      return <SettingsScreen />;
  }
}

export default function App() {
  return (
    <div className="flex flex-col h-full">
      <TitleBar />
      <div className="flex flex-1 min-h-0">
        <Sidebar />
        <main className="flex-1 min-w-0 overflow-y-auto" style={{ background: "var(--bg-void)" }}>
          <ActiveScreen />
        </main>
      </div>
      <ToastContainer />
    </div>
  );
}
