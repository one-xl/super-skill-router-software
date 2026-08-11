<script setup lang="ts">
import { onMounted, ref } from "vue";
import { Check, KeyRound, LoaderCircle, Save, Settings2 } from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";
import type { ApiConfig, AppSettings } from "../types/skill";

const empty = (): ApiConfig => ({ format: "openai", apiUrl: "", apiKey: "", model: "", apiKeyConfigured: false });
const settings = ref<AppSettings>({ deepScan: empty(), prompt: empty(), skillsMp: { apiKey: "", apiKeyConfigured: false } });
const loading = ref(true);
const saving = ref(false);
const saved = ref(false);
const error = ref<string | null>(null);
const apiSections = [
  { key: "deepScan" as const, title: "深度扫描模型", hint: "用于 SkillSpector 的 LLM 语义分析。" },
  { key: "prompt" as const, title: "Prompt 精炼模型", hint: "用于把模板 Prompt 精炼成可直接执行的版本。" },
];

async function load() {
  loading.value = true;
  try {
    settings.value = await invoke<AppSettings>("get_settings");
  } catch (cause) {
    error.value = String(cause);
  } finally {
    loading.value = false;
  }
}

async function save() {
  saving.value = true;
  error.value = null;
  try {
    await invoke("save_settings", { settings: settings.value });
    settings.value = await invoke<AppSettings>("get_settings");
    saved.value = true;
    window.setTimeout(() => { saved.value = false; }, 1600);
  } catch (cause) {
    error.value = String(cause);
  } finally {
    saving.value = false;
  }
}

onMounted(() => { void load(); });
</script>

<template>
  <section class="mx-auto w-full max-w-4xl px-6 py-8 lg:px-10">
    <div class="mb-7">
      <p class="mb-2 text-xs font-semibold uppercase tracking-[0.18em] text-teal-700">Settings</p>
      <h1 class="text-3xl font-semibold text-slate-950">模型与远程源</h1>
      <p class="mt-2 text-sm text-slate-500">深度扫描、Prompt 精炼与 SkillsMP 分开配置。密钥存入 Windows Credential Manager，不写入设置文件。</p>
    </div>
    <p v-if="error" class="mb-4 border-l-4 border-rose-500 bg-rose-50 p-3 text-sm text-rose-900">{{ error }}</p>
    <div v-if="loading" class="flex items-center gap-2 text-sm text-slate-500"><LoaderCircle class="size-4 animate-spin" />正在读取设置</div>
    <div v-else class="space-y-5">
      <section v-for="section in apiSections" :key="section.key" class="border border-slate-200 bg-white p-5">
        <h2 class="flex items-center gap-2 font-semibold text-slate-900"><Settings2 class="size-4 text-teal-700" />{{ section.title }}</h2>
        <p class="mt-1 text-xs text-slate-500">{{ section.hint }}</p>
        <div class="mt-4 grid gap-4 sm:grid-cols-2">
          <label class="text-sm text-slate-700">接口格式<select v-model="settings[section.key].format" class="mt-1 h-10 w-full border border-slate-300 bg-white px-2"><option value="openai">OpenAI 兼容</option><option value="anthropic">Anthropic Messages</option></select></label>
          <label class="text-sm text-slate-700">模型（可选）<input v-model="settings[section.key].model" class="mt-1 h-10 w-full border border-slate-300 px-2" placeholder="例如 gpt-4o-mini" /></label>
          <label class="text-sm text-slate-700 sm:col-span-2">API URL<input v-model="settings[section.key].apiUrl" class="mt-1 h-10 w-full border border-slate-300 px-2" placeholder="https://api.openai.com/v1" /></label>
          <label class="text-sm text-slate-700 sm:col-span-2">API Key <span v-if="settings[section.key].apiKeyConfigured" class="text-xs text-emerald-700">已安全保存，留空则保持不变</span><input v-model="settings[section.key].apiKey" type="password" class="mt-1 h-10 w-full border border-slate-300 px-2" autocomplete="off" placeholder="输入新密钥以替换" /></label>
        </div>
      </section>
      <section class="border border-slate-200 bg-white p-5">
        <h2 class="flex items-center gap-2 font-semibold text-slate-900"><KeyRound class="size-4 text-teal-700" />SkillsMP 远程发现</h2>
        <p class="mt-1 text-xs text-slate-500">仅用于用户主动发起的远程搜索。下载会回到 GitHub，以 commit SHA 提取完整 skill 目录。</p>
        <label class="mt-4 block text-sm text-slate-700">SkillsMP API Key <span v-if="settings.skillsMp.apiKeyConfigured" class="text-xs text-emerald-700">已安全保存，留空则保持不变</span><input v-model="settings.skillsMp.apiKey" type="password" class="mt-1 h-10 w-full border border-slate-300 px-2" autocomplete="off" placeholder="输入 SkillsMP API Key" /></label>
      </section>
      <button type="button" class="inline-flex h-10 items-center gap-2 bg-teal-600 px-4 text-sm font-medium text-white disabled:bg-slate-300" :disabled="saving" @click="save"><LoaderCircle v-if="saving" class="size-4 animate-spin" /><Check v-else-if="saved" class="size-4" /><Save v-else class="size-4" />{{ saved ? '已保存' : '保存设置' }}</button>
    </div>
  </section>
</template>
