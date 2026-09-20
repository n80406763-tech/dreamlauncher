import { useState, useEffect } from "react";
import { Panel } from "../components/ui/Panel";
import { Button } from "../components/ui/Button";
import { open } from "@tauri-apps/plugin-shell";
import { appDataDir } from "@tauri-apps/api/path";
import { check } from "@tauri-apps/plugin-updater";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";

export function SettingsScreen() {
  const [dataPath, setDataPath] = useState<string>("");

  useEffect(() => {
    appDataDir().then(setDataPath).catch(console.error);
  }, []);

  const openDataFolder = async () => {
    if (dataPath) {
      await open(dataPath);
    }
  };

  return (
    <div className="flex flex-col gap-4 p-6 max-w-2xl">
      <h1 className="font-pixel text-lg" style={{ fontFamily: "var(--font-pixel)", color: "var(--text-hi)" }}>
        Настройки
      </h1>

      {/* Пути к данным */}
      <Panel className="flex flex-col gap-3">
        <div className="font-pixel text-sm" style={{ fontFamily: "var(--font-pixel)", color: "var(--text-hi)" }}>
          Пути к данным
        </div>
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between gap-4">
            <div className="flex flex-col gap-1 flex-1">
              <div className="text-sm" style={{ color: "var(--text-hi)" }}>Папка приложения</div>
              <div className="text-xs font-mono" style={{ color: "var(--text-dim)" }}>
                {dataPath || "Загрузка..."}
              </div>
            </div>
            <Button onClick={openDataFolder} disabled={!dataPath}>
              Открыть
            </Button>
          </div>
          <div className="text-xs" style={{ color: "var(--text-dim)" }}>
            Здесь хранятся инстансы, версии игры, библиотеки, ассеты и Java
          </div>
        </div>
      </Panel>

      {/* Производительность */}
      <Panel className="flex flex-col gap-3">
        <div className="font-pixel text-sm" style={{ fontFamily: "var(--font-pixel)", color: "var(--text-hi)" }}>
          Производительность
        </div>
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between gap-4">
            <div className="flex flex-col gap-1">
              <div className="text-sm" style={{ color: "var(--text-hi)" }}>Параллельных загрузок</div>
              <div className="text-xs" style={{ color: "var(--text-dim)" }}>По умолчанию: 16</div>
            </div>
            <div className="text-sm" style={{ color: "var(--text-dim)" }}>
              Настройка появится в M9
            </div>
          </div>
        </div>
      </Panel>

      {/* Внешний вид и язык */}
      <Panel className="flex flex-col gap-3">
        <div className="font-pixel text-sm" style={{ fontFamily: "var(--font-pixel)", color: "var(--text-hi)" }}>
          Внешний вид и Язык
        </div>
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between gap-4">
            <div className="flex flex-col gap-1">
              <div className="text-sm" style={{ color: "var(--text-hi)" }}>Язык интерфейса</div>
              <div className="text-xs" style={{ color: "var(--text-dim)" }}>Выберите язык / Select language</div>
            </div>
            <select className="field" onChange={(_e) => {}} defaultValue="ru">
              <option value="ru">Русский</option>
              <option value="en">English</option>
            </select>
          </div>
          
          <div className="flex items-center justify-between gap-4 mt-2">
            <div className="flex flex-col gap-1">
              <div className="text-sm" style={{ color: "var(--text-hi)" }}>Тема оформления</div>
              <div className="text-xs" style={{ color: "var(--text-dim)" }}>Темная тема с космическими акцентами</div>
            </div>
            <div className="text-sm" style={{ color: "var(--text-dim)" }}>
              Авто
            </div>
          </div>
        </div>
      </Panel>

      {/* Обновления */}
      <Panel className="flex flex-col gap-3">
        <div className="font-pixel text-sm" style={{ fontFamily: "var(--font-pixel)", color: "var(--text-hi)" }}>
          Обновления
        </div>
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between gap-4">
            <div className="flex flex-col gap-1">
              <div className="text-sm" style={{ color: "var(--text-hi)" }}>Версия приложения</div>
              <div className="text-xs" style={{ color: "var(--text-dim)" }}>Проверка наличия новых версий лаунчера</div>
            </div>
            <Button onClick={async () => {
              try {
                const update = await check();
                if (update) {
                  const yes = await ask(`Доступно обновление ${update.version}.\n\nХотите установить его сейчас?`, { title: "Обновление", kind: "info" });
                  if (yes) {
                    await update.downloadAndInstall((_event) => {
                      // could log progress
                    });
                    await relaunch();
                  }
                } else {
                  await message("У вас установлена самая последняя версия.", { title: "Обновлений нет", kind: "info" });
                }
              } catch (e) {
                await message(`Ошибка при проверке обновлений: ${e}`, { title: "Ошибка", kind: "error" });
              }
            }}>
              Проверить обновления
            </Button>
          </div>
        </div>
      </Panel>

      {/* О программе */}
      <Panel className="flex flex-col gap-2">
        <div className="font-pixel text-sm" style={{ fontFamily: "var(--font-pixel)", color: "var(--text-hi)" }}>
          О программе
        </div>
        <div style={{ color: "var(--text-dim)" }}>DreamLauncher v0.1.0 — сделано командой NetRender.</div>
        <div className="text-xs" style={{ color: "var(--text-dim)" }}>Не является продуктом Mojang или Microsoft.</div>
        <div className="flex gap-2 mt-2">
          <Button
            variant="secondary"
            onClick={() => open("https://github.com/netrender/dreamlauncher")}
          >
            GitHub
          </Button>
          <Button
            variant="secondary"
            onClick={() => open("https://netrender.org")}
          >
            NetRender
          </Button>
        </div>
      </Panel>
    </div>
  );
}
