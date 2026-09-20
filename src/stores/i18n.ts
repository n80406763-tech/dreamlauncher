import { create } from "zustand";

export type Language = "ru" | "en";

interface Translations {
  [key: string]: {
    ru: string;
    en: string;
  };
}

const translations: Translations = {
  // Sidebar
  nav_play: { ru: "Играть", en: "Play" },
  nav_instances: { ru: "Корабли", en: "Ships" },
  nav_accounts: { ru: "Пилот", en: "Pilot" },
  nav_settings: { ru: "Настройки", en: "Settings" },
  
  // Settings
  settings_title: { ru: "Настройки", en: "Settings" },
  settings_data_path: { ru: "Пути к данным", en: "Data Paths" },
  settings_app_folder: { ru: "Папка приложения", en: "App Folder" },
  settings_open: { ru: "Открыть", en: "Open" },
  settings_performance: { ru: "Производительность", en: "Performance" },
  settings_appearance: { ru: "Внешний вид", en: "Appearance" },
  settings_updates: { ru: "Обновления", en: "Updates" },
  settings_check_updates: { ru: "Проверить обновления", en: "Check for Updates" },
  settings_about: { ru: "О программе", en: "About" },
  settings_language: { ru: "Язык (Language)", en: "Language (Язык)" },
};

interface I18nStore {
  lang: Language;
  setLang: (lang: Language) => void;
  t: (key: keyof typeof translations) => string;
}

export const useI18nStore = create<I18nStore>((set, get) => ({
  lang: "ru",
  setLang: (lang) => set({ lang }),
  t: (key) => {
    const entry = translations[key];
    if (!entry) return String(key);
    return entry[get().lang];
  },
}));
