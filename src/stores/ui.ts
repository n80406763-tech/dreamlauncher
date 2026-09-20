// UI-состояние, которое не приходит с бэкенда и не нужно кэшировать через
// TanStack Query — просто то, какой экран сейчас открыт. Полноценный роутер
// (TanStack Router) стоит подключать, когда появятся вложенные экраны
// инстанса (Обзор/Моды/Логи и т.п.) — пока четырёх плоских вкладок хватает.
import { create } from "zustand";

export type Screen = "play" | "instances" | "accounts" | "settings";

interface UiState {
  screen: Screen;
  setScreen: (screen: Screen) => void;
}

export const useUiStore = create<UiState>((set) => ({
  screen: "play",
  setScreen: (screen) => set({ screen }),
}));
