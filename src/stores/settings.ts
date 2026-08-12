import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import type { AppSettings } from "../types/skill";

const defaultSettings = (): AppSettings => ({
  deepScan: { format: "openai", apiUrl: "", apiKey: "", model: "", apiKeyConfigured: false },
  prompt: { format: "openai", apiUrl: "", apiKey: "", model: "", apiKeyConfigured: false },
  skillsMp: { apiKey: "", apiKeyConfigured: false },
  automation: { autoInjectAfterRefine: false, startCodexRecoveryMonitorOnLaunch: false, recoveryText: "继续并恢复todo-list" },
});

export const useSettingsStore = defineStore("settings", () => {
  const settings = ref<AppSettings>(defaultSettings());
  const loading = ref(false);
  const loaded = ref(false);
  const saving = ref(false);
  const error = ref<string | null>(null);
  const autoInjectAfterRefine = computed(() => settings.value.automation.autoInjectAfterRefine);

  async function load() {
    if (loading.value) return;
    loading.value = true;
    error.value = null;
    try {
      settings.value = await invoke<AppSettings>("get_settings");
      loaded.value = true;
    } catch (cause) {
      error.value = typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "无法读取设置";
      throw cause;
    } finally {
      loading.value = false;
    }
  }

  async function save(next?: AppSettings) {
    const previous = structuredClone(settings.value);
    if (next) settings.value = structuredClone(next);
    saving.value = true;
    error.value = null;
    try {
      await invoke("save_settings", { settings: settings.value });
      settings.value = await invoke<AppSettings>("get_settings");
      loaded.value = true;
      window.dispatchEvent(new CustomEvent("super-skill-router:settings-updated"));
    } catch (cause) {
      settings.value = previous;
      error.value = typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "无法保存设置";
      throw cause;
    } finally {
      saving.value = false;
    }
  }

  async function setAutoInjectAfterRefine(value: boolean) {
    const next = structuredClone(settings.value);
    next.automation.autoInjectAfterRefine = value;
    await save(next);
  }

  return { settings, loading, loaded, saving, error, autoInjectAfterRefine, load, save, setAutoInjectAfterRefine };
});
