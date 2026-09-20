import { useState } from "react";
import { Channel } from "@tauri-apps/api/core";
import { Button } from "../components/ui/Button";
import { Panel } from "../components/ui/Panel";
import { useAccounts, useAddOfflineAccount, useRemoveAccount, useSetActiveAccount } from "../hooks/queries";
import { commands, type AuthProgressDto } from "../ipc/bindings";

export function AccountsScreen() {
  const { data: accounts, isLoading } = useAccounts();
  const addOffline = useAddOfflineAccount();
  const setActive = useSetActiveAccount();
  const remove = useRemoveAccount();
  const [name, setName] = useState("");
  const [msAuthState, setMsAuthState] = useState<{ status: "idle" | "working"; progress?: AuthProgressDto }>({ status: "idle" });

  const nameError =
    name.length > 0 && !(name.length >= 3 && name.length <= 16 && /^[A-Za-z0-9_]+$/.test(name))
      ? "3-16 символов: латиница, цифры, подчёркивание"
      : null;

  const handleAddMicrosoft = async () => {
    setMsAuthState({ status: "working" });
    const channel = new Channel<AuthProgressDto>();
    channel.onmessage = (progress) => {
      setMsAuthState({ status: "working", progress });
    };

    try {
      await commands.accountAddMicrosoft(channel);
      setMsAuthState({ status: "idle" });
    } catch (error) {
      setMsAuthState({ status: "idle", progress: { type: "Error", message: String(error) } });
    }
  };

  return (
    <div className="flex flex-col gap-4 p-6 max-w-2xl">
      <h1 className="font-pixel text-lg" style={{ fontFamily: "var(--font-pixel)", color: "var(--text-hi)" }}>
        Аккаунты
      </h1>

      <Panel className="flex flex-col gap-3">
        <div className="text-sm font-semibold" style={{ color: "var(--text-hi)" }}>
          Microsoft-аккаунт
        </div>
        <div className="text-sm" style={{ color: "var(--text-dim)" }}>
          Для игры на серверах с лицензией. Требуется регистрация Azure-приложения (CLIENT_ID в переменных окружения).
        </div>
        <Button variant="primary" disabled={msAuthState.status === "working"} onClick={handleAddMicrosoft}>
          {msAuthState.status === "working" ? "Авторизация…" : "Добавить Microsoft-аккаунт"}
        </Button>
        {msAuthState.progress?.type === "WaitingForUser" ? (
          <div className="flex flex-col gap-2 p-3" style={{ background: "var(--bg-slot)" }}>
            <div className="text-sm" style={{ color: "var(--text-hi)" }}>
              Перейдите по ссылке и введите код:
            </div>
            <div className="flex items-center gap-2">
              <code className="text-lg font-bold" style={{ fontFamily: "var(--font-pixel)", color: "var(--xp)" }}>
                {msAuthState.progress.user_code}
              </code>
              <Button
                size="sm"
                onClick={() => {
                  if (msAuthState.progress?.type === "WaitingForUser") {
                    window.open(msAuthState.progress.verification_uri, "_blank");
                  }
                }}
              >
                Открыть
              </Button>
            </div>
            <div className="text-xs" style={{ color: "var(--text-dim)" }}>
              Истекает через {msAuthState.progress.expires_in} секунд
            </div>
          </div>
        ) : null}
        {msAuthState.progress?.type === "Polling" ? (
          <div className="text-sm" style={{ color: "var(--text-dim)" }}>
            Ожидание подтверждения…
          </div>
        ) : null}
        {msAuthState.progress?.type === "ExchangingXboxLive" ? (
          <div className="text-sm" style={{ color: "var(--text-dim)" }}>
            Обмен токена Xbox Live…
          </div>
        ) : null}
        {msAuthState.progress?.type === "ExchangingXsts" ? (
          <div className="text-sm" style={{ color: "var(--text-dim)" }}>
            Получение XSTS токена…
          </div>
        ) : null}
        {msAuthState.progress?.type === "LoggingIntoMinecraft" ? (
          <div className="text-sm" style={{ color: "var(--text-dim)" }}>
            Вход в Minecraft Services…
          </div>
        ) : null}
        {msAuthState.progress?.type === "FetchingProfile" ? (
          <div className="text-sm" style={{ color: "var(--text-dim)" }}>
            Загрузка профиля…
          </div>
        ) : null}
        {msAuthState.progress?.type === "Complete" ? (
          <div className="text-sm" style={{ color: "var(--xp)" }}>
            ✓ Успешно: {msAuthState.progress.username}
          </div>
        ) : null}
        {msAuthState.progress?.type === "Error" ? (
          <div className="text-sm" style={{ color: "var(--danger-hi)" }}>
            Ошибка: {msAuthState.progress.message}
          </div>
        ) : null}
      </Panel>

      <Panel className="flex flex-col gap-3">
        <div className="text-sm font-semibold" style={{ color: "var(--text-hi)" }}>
          Офлайн-профиль
        </div>
        <div className="text-sm" style={{ color: "var(--text-dim)" }}>
          Для одиночной игры и серверов с <code>online-mode=false</code>.
        </div>
        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (!nameError && name) addOffline.mutate(name, { onSuccess: () => setName("") });
          }}
        >
          <input
            className="field flex-1"
            placeholder="Имя игрока"
            value={name}
            onChange={(e) => setName(e.target.value)}
            maxLength={16}
          />
          <Button type="submit" variant="primary" disabled={!name || !!nameError || addOffline.isPending}>
            {addOffline.isPending ? "Добавляем…" : "Добавить"}
          </Button>
        </form>
        {nameError ? (
          <div className="text-xs" style={{ color: "var(--danger-hi)" }}>
            {nameError}
          </div>
        ) : null}
        {addOffline.isError ? (
          <div className="text-xs" style={{ color: "var(--danger-hi)" }}>
            {addOffline.error.message}
          </div>
        ) : null}
      </Panel>

      <Panel className="flex flex-col gap-2">
        <div className="text-sm font-semibold mb-2" style={{ color: "var(--text-hi)" }}>
          Список аккаунтов
        </div>
        {isLoading ? <div style={{ color: "var(--text-dim)" }}>Загрузка…</div> : null}
        {accounts?.length === 0 ? <div style={{ color: "var(--text-dim)" }}>Аккаунтов пока нет.</div> : null}
        {accounts?.map((acc) => (
          <div key={acc.id} className="flex items-center justify-between p-2" style={{ background: acc.is_active ? "var(--bg-stone-2)" : "transparent" }}>
            <div className="flex items-center gap-2">
              <span>{acc.username}</span>
              <span className="badge">{acc.kind === "microsoft" ? "Microsoft" : "Офлайн"}</span>
              {acc.is_active ? (
                <span className="badge badge--release">Активен</span>
              ) : null}
            </div>
            <div className="flex gap-2">
              {!acc.is_active ? (
                <Button size="sm" onClick={() => setActive.mutate(acc.id)}>
                  Сделать активным
                </Button>
              ) : null}
              <Button size="sm" variant="danger" onClick={() => remove.mutate(acc.id)}>
                Удалить
              </Button>
            </div>
          </div>
        ))}
      </Panel>
    </div>
  );
}
