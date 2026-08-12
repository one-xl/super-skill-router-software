<script setup lang="ts">
import { computed, onMounted, ref, type Component } from "vue";
import {
  Braces,
  Database,
  ExternalLink,
  KeyRound,
  LibraryBig,
  MonitorDot,
  Search,
  Settings,
  X,
} from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import appIcon from "./assets/app-icon.svg";
import Discover from "./views/Discover.vue";
import Compose from "./views/Compose.vue";
import Manage from "./views/Manage.vue";
import LocalIndex from "./views/LocalIndex.vue";
import MonitorView from "./views/Monitor.vue";
import SettingsView from "./views/Settings.vue";
import type { AppSettings } from "./types/skill";

type ViewId = "discover" | "compose" | "manage" | "settings" | "index" | "monitor";

const SETUP_DISMISSED_KEY = "super-skill-router:skillsmp-setup-dismissed";
const SKILLSMP_DOCS_URL = "https://skillsmp.com/zh/docs/api";
const view = ref<ViewId>("discover");
const showSkillsMpSetup = ref(false);
const viewComponents: Record<ViewId, Component> = {
  discover: Discover,
  compose: Compose,
  manage: Manage,
  index: LocalIndex,
  monitor: MonitorView,
  settings: SettingsView,
};
const activeComponent = computed(() => viewComponents[view.value]);

const primaryNavigation = [
  { id: "discover" as const, label: "发现", hint: "搜索与部署", icon: Search },
  { id: "compose" as const, label: "转换器", hint: "需求转 Prompt", icon: Braces },
  { id: "manage" as const, label: "管理", hint: "跨端同步", icon: LibraryBig },
];

const activeLabel = computed(() => {
  const labels: Record<ViewId, string> = {
    discover: "技能发现",
    compose: "需求转 Prompt",
    manage: "Skill 管理",
    index: "本地索引",
    monitor: "Agent 监控",
    settings: "设置",
  };
  return labels[view.value];
});

async function initializeApp() {
  try {
    const settings = await invoke<AppSettings>("get_settings");
    showSkillsMpSetup.value = !settings.skillsMp.apiKeyConfigured && localStorage.getItem(SETUP_DISMISSED_KEY) !== "1";
    if (settings.automation.startCodexRecoveryMonitorOnLaunch) {
      await startDesktopRecoveryMonitor();
    }
  } catch {
    // The settings page presents the concrete error if configuration cannot be read.
  }
}

async function startDesktopRecoveryMonitor() {
  try {
    const monitors = await invoke<Array<{ target_id: string }>>("list_desktop_monitors");
    if (!monitors.some((monitor) => monitor.target_id === "codex_desktop")) {
      await invoke("start_desktop_monitor", { targetId: "codex_desktop" });
    }
  } catch {
    // The monitor view exposes log discovery and desktop automation errors.
  }
}

function configureSkillsMp() {
  showSkillsMpSetup.value = false;
  view.value = "settings";
}

function postponeSkillsMpSetup() {
  localStorage.setItem(SETUP_DISMISSED_KEY, "1");
  showSkillsMpSetup.value = false;
}

async function openSkillsMpDocs() {
  await openUrl(SKILLSMP_DOCS_URL);
}

onMounted(() => {
  void initializeApp();
});
</script>

<template>
  <div class="app-shell">
    <aside class="app-sidebar">
      <div class="brand-lockup">
        <img :src="appIcon" class="brand-mark" alt="" />
        <div class="min-w-0">
          <p class="brand-name">Super Skill Router</p>
          <p class="brand-edition">Windows Desktop</p>
        </div>
      </div>

      <nav class="app-navigation" aria-label="主导航">
        <p class="nav-section-label">工作区</p>
        <button
          v-for="item in primaryNavigation"
          :key="item.id"
          type="button"
          class="nav-item"
          :class="view === item.id && 'nav-item-active'"
          :aria-current="view === item.id ? 'page' : undefined"
          @click="view = item.id"
        >
          <component :is="item.icon" class="nav-icon" :stroke-width="1.8" />
          <span class="min-w-0 flex-1">
            <span class="nav-title">{{ item.label }}</span>
            <span class="nav-hint">{{ item.hint }}</span>
          </span>
        </button>

        <p class="nav-section-label mt-6">数据</p>
        <button
          type="button"
          class="nav-item"
          :class="view === 'index' && 'nav-item-active'"
          :aria-current="view === 'index' ? 'page' : undefined"
          @click="view = 'index'"
        >
          <Database class="nav-icon" :stroke-width="1.8" />
          <span class="min-w-0 flex-1">
            <span class="nav-title">本地索引</span>
            <span class="nav-hint">离线数据源</span>
          </span>
        </button>
        <button
          type="button"
          class="nav-item"
          :class="view === 'monitor' && 'nav-item-active'"
          :aria-current="view === 'monitor' ? 'page' : undefined"
          @click="view = 'monitor'"
        >
          <MonitorDot class="nav-icon" :stroke-width="1.8" />
          <span class="min-w-0 flex-1">
            <span class="nav-title">监控</span>
            <span class="nav-hint">重连自动恢复</span>
          </span>
        </button>
      </nav>

      <div class="sidebar-footer">
        <button
          type="button"
          class="nav-item"
          :class="view === 'settings' && 'nav-item-active'"
          :aria-current="view === 'settings' ? 'page' : undefined"
          @click="view = 'settings'"
        >
          <Settings class="nav-icon" :stroke-width="1.8" />
          <span class="min-w-0 flex-1">
            <span class="nav-title">设置</span>
            <span class="nav-hint">API 与远程源</span>
          </span>
        </button>
      </div>
    </aside>

    <div class="app-workspace">
      <header class="workspace-bar">
        <span class="workspace-title">{{ activeLabel }}</span>
        <span class="workspace-status"><span class="status-dot" />本机模式</span>
      </header>
      <main class="workspace-content">
        <Transition name="page" mode="out-in">
          <KeepAlive>
            <component :is="activeComponent" :key="view" />
          </KeepAlive>
        </Transition>
      </main>
    </div>

    <div v-if="showSkillsMpSetup" class="modal-backdrop" role="dialog" aria-modal="true" aria-label="SkillsMP 配置建议">
      <section class="modal-panel max-w-md">
        <div class="flex items-start justify-between gap-4">
          <div class="modal-icon-title"><span class="modal-icon"><KeyRound class="size-5" /></span><span>连接 SkillsMP</span></div>
          <button class="icon-button" type="button" title="稍后配置" @click="postponeSkillsMpSetup"><X class="size-4" /></button>
        </div>
        <p class="mt-4 text-sm leading-6 text-stone-600">配置 API Key 后可按需搜索远程 skill。跳过不会影响本地索引、扫描、安装和管理功能。</p>
        <div class="mt-6 flex flex-wrap justify-end gap-2">
          <button type="button" class="button-secondary" @click="openSkillsMpDocs"><ExternalLink class="size-4" />获取 API Key</button>
          <button type="button" class="button-ghost" @click="postponeSkillsMpSetup">稍后配置</button>
          <button type="button" class="button-primary" @click="configureSkillsMp">前往设置</button>
        </div>
      </section>
    </div>
  </div>
</template>
