<script setup lang="ts">
import { onMounted, ref } from "vue";
import { Database, ExternalLink, KeyRound, LayoutList, Network, Settings, Wand2, X } from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import Discover from "./views/Discover.vue";
import Compose from "./views/Compose.vue";
import Manage from "./views/Manage.vue";
import SettingsView from "./views/Settings.vue";
import type { AppSettings } from "./types/skill";

const SETUP_DISMISSED_KEY = "super-skill-router:skillsmp-setup-dismissed";
const SKILLSMP_DOCS_URL = "https://skillsmp.com/zh/docs/api";
const view = ref<"discover" | "compose" | "manage" | "settings">("discover");
const showSkillsMpSetup = ref(false);

async function checkFirstRunSetup() {
  try {
    const settings = await invoke<AppSettings>("get_settings");
    showSkillsMpSetup.value = !settings.skillsMp.apiKeyConfigured && localStorage.getItem(SETUP_DISMISSED_KEY) !== "1";
  } catch {
    // The settings page will present the concrete error if configuration cannot be read.
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

onMounted(() => { void checkFirstRunSetup(); });
</script>

<template>
  <div class="min-h-screen bg-slate-50 text-slate-900">
    <header class="border-b border-slate-200 bg-white">
      <div class="mx-auto flex h-16 max-w-6xl items-center justify-between px-6 lg:px-10">
        <div class="flex items-center gap-3"><div class="flex size-8 items-center justify-center rounded-lg bg-teal-600 text-white"><Network class="size-4" :stroke-width="2" /></div><span class="text-sm font-semibold text-slate-950">Super Skill Router</span></div>
        <div class="flex items-center gap-4"><nav class="flex items-center gap-1 text-sm"><button type="button" class="h-8 px-2 text-slate-500 hover:text-teal-700" :class="view === 'discover' && 'font-semibold text-teal-700'" @click="view = 'discover'">发现</button><button type="button" class="inline-flex h-8 items-center gap-1 px-2 text-slate-500 hover:text-teal-700" :class="view === 'compose' && 'font-semibold text-teal-700'" @click="view = 'compose'"><Wand2 class="size-3.5" />转换器</button><button type="button" class="inline-flex h-8 items-center gap-1 px-2 text-slate-500 hover:text-teal-700" :class="view === 'manage' && 'font-semibold text-teal-700'" @click="view = 'manage'"><LayoutList class="size-3.5" />管理</button><button type="button" class="flex size-8 items-center justify-center text-slate-500 hover:text-teal-700" :class="view === 'settings' && 'text-teal-700'" title="设置" @click="view = 'settings'"><Settings class="size-4" /></button></nav><div class="inline-flex items-center gap-2 text-xs text-slate-500"><Database class="size-3.5 text-teal-600" />本地索引</div></div>
      </div>
    </header>
    <main><Discover v-if="view === 'discover'" /><Compose v-else-if="view === 'compose'" /><Manage v-else-if="view === 'manage'" /><SettingsView v-else /></main>

    <div v-if="showSkillsMpSetup" class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/35 p-5" role="dialog" aria-modal="true" aria-label="SkillsMP 配置建议">
      <section class="w-full max-w-md border border-slate-200 bg-white p-6 shadow-xl">
        <div class="flex items-start justify-between gap-4"><div class="flex items-center gap-2 text-base font-semibold text-slate-950"><KeyRound class="size-5 text-teal-700" />连接 SkillsMP</div><button class="flex size-8 items-center justify-center text-slate-400 hover:bg-slate-100 hover:text-slate-700" type="button" title="稍后配置" @click="postponeSkillsMpSetup"><X class="size-4" /></button></div>
        <p class="mt-3 text-sm leading-6 text-slate-600">配置 API Key 后可按需搜索远程 skill。跳过不会影响本地索引、扫描、安装和管理功能。</p>
        <div class="mt-5 flex flex-wrap justify-end gap-2"><button type="button" class="inline-flex h-9 items-center gap-2 border border-slate-300 bg-white px-3 text-sm text-slate-700 hover:border-teal-400 hover:text-teal-800" @click="openSkillsMpDocs"><ExternalLink class="size-4" />获取 API Key</button><button type="button" class="h-9 border border-slate-300 bg-white px-3 text-sm text-slate-700" @click="postponeSkillsMpSetup">稍后配置</button><button type="button" class="h-9 bg-teal-600 px-3 text-sm font-medium text-white hover:bg-teal-700" @click="configureSkillsMp">前往设置</button></div>
      </section>
    </div>
  </div>
</template>
