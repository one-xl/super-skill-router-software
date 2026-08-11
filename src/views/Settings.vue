<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Check, CircleAlert, CircleCheck, ExternalLink, KeyRound, LoaderCircle, Save, Settings2 } from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { ApiConfig, AppSettings } from "../types/skill";

const SKILLSMP_DOCS_URL = "https://skillsmp.com/zh/docs/api";
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

function apiConfigured(config: ApiConfig) {
  return Boolean(config.apiKeyConfigured && config.apiUrl.trim());
}

const configurationSummary = computed(() => [
  { label: "SkillsMP 远程发现", configured: Boolean(settings.value.skillsMp.apiKeyConfigured), suggested: true },
  { label: "深度扫描", configured: apiConfigured(settings.value.deepScan), suggested: false },
  { label: "Prompt 精炼", configured: apiConfigured(settings.value.prompt), suggested: false },
]);

async function load() {
  loading.value = true;
  try { settings.value = await invoke<AppSettings>("get_settings"); }
  catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}

async function save() {
  saving.value = true;
  error.value = null;
  try {
    await invoke("save_settings", { settings: settings.value });
    settings.value = await invoke<AppSettings>("get_settings");
    saved.value = true;
    window.setTimeout(() => { saved.value = false; }, 1600);
  } catch (cause) { error.value = String(cause); }
  finally { saving.value = false; }
}

async function openSkillsMpDocs() {
  await openUrl(SKILLSMP_DOCS_URL);
}

onMounted(() => { void load(); });
</script>

<template>
  <section class="mx-auto w-full max-w-4xl px-6 py-8 lg:px-10">
    <div class="mb-7"><p class="mb-2 text-xs font-semibold uppercase tracking-[0.18em] text-teal-700">Settings</p><h1 class="text-3xl font-semibold text-slate-950">模型与远程源</h1><p class="mt-2 text-sm text-slate-500">所有配置均为可选。密钥存入 Windows Credential Manager，不写入设置文件。</p></div>
    <p v-if="error" class="mb-4 border-l-4 border-rose-500 bg-rose-50 p-3 text-sm text-rose-900">{{ error }}</p>
    <div v-if="loading" class="flex items-center gap-2 text-sm text-slate-500"><LoaderCircle class="size-4 animate-spin" />正在读取设置</div>
    <div v-else class="space-y-5">
      <div class="grid border border-slate-200 bg-white sm:grid-cols-3"><div v-for="item in configurationSummary" :key="item.label" class="flex items-center justify-between gap-3 border-b border-slate-200 p-4 last:border-b-0 sm:border-b-0 sm:border-r sm:last:border-r-0"><div><p class="text-sm font-medium text-slate-800">{{ item.label }}</p><p class="mt-1 text-xs text-slate-500">{{ item.suggested ? '建议配置，用于远程发现' : '可选功能' }}</p></div><span class="inline-flex shrink-0 items-center gap-1 text-xs font-medium" :class="item.configured ? 'text-emerald-700' : 'text-slate-500'"><CircleCheck v-if="item.configured" class="size-4" /><CircleAlert v-else class="size-4" />{{ item.configured ? '已配置' : '未配置' }}</span></div></div>
      <section v-for="section in apiSections" :key="section.key" class="border border-slate-200 bg-white p-5"><div class="flex flex-wrap items-start justify-between gap-3"><div><h2 class="flex items-center gap-2 font-semibold text-slate-900"><Settings2 class="size-4 text-teal-700" />{{ section.title }}</h2><p class="mt-1 text-xs text-slate-500">{{ section.hint }}</p></div><span class="inline-flex items-center gap-1 text-xs font-medium" :class="apiConfigured(settings[section.key]) ? 'text-emerald-700' : 'text-slate-500'"><CircleCheck v-if="apiConfigured(settings[section.key])" class="size-4" /><CircleAlert v-else class="size-4" />{{ apiConfigured(settings[section.key]) ? '已配置' : '未配置（可选）' }}</span></div><div class="mt-4 grid gap-4 sm:grid-cols-2"><label class="text-sm text-slate-700">接口格式<select v-model="settings[section.key].format" class="mt-1 h-10 w-full border border-slate-300 bg-white px-2"><option value="openai">OpenAI 兼容</option><option value="anthropic">Anthropic Messages</option></select></label><label class="text-sm text-slate-700">模型（可选）<input v-model="settings[section.key].model" class="mt-1 h-10 w-full border border-slate-300 px-2" placeholder="例如 gpt-4o-mini" /></label><label class="text-sm text-slate-700 sm:col-span-2">API URL<input v-model="settings[section.key].apiUrl" class="mt-1 h-10 w-full border border-slate-300 px-2" placeholder="https://api.openai.com/v1" /></label><label class="text-sm text-slate-700 sm:col-span-2">API Key <span v-if="settings[section.key].apiKeyConfigured" class="text-xs text-emerald-700">已安全保存，留空则保持不变</span><input v-model="settings[section.key].apiKey" type="password" class="mt-1 h-10 w-full border border-slate-300 px-2" autocomplete="off" placeholder="输入新密钥以替换" /></label></div></section>
      <section class="border border-slate-200 bg-white p-5"><div class="flex flex-wrap items-start justify-between gap-3"><div><h2 class="flex items-center gap-2 font-semibold text-slate-900"><KeyRound class="size-4 text-teal-700" />SkillsMP 远程发现</h2><p class="mt-1 text-xs text-slate-500">用于用户主动发起的远程搜索；下载仍以 commit SHA 提取完整 skill 目录。</p></div><span class="inline-flex items-center gap-1 text-xs font-medium" :class="settings.skillsMp.apiKeyConfigured ? 'text-emerald-700' : 'text-slate-500'"><CircleCheck v-if="settings.skillsMp.apiKeyConfigured" class="size-4" /><CircleAlert v-else class="size-4" />{{ settings.skillsMp.apiKeyConfigured ? '已配置' : '未配置（建议）' }}</span></div><label class="mt-4 block text-sm text-slate-700">SkillsMP API Key <span v-if="settings.skillsMp.apiKeyConfigured" class="text-xs text-emerald-700">已安全保存，留空则保持不变</span><input v-model="settings.skillsMp.apiKey" type="password" class="mt-1 h-10 w-full border border-slate-300 px-2" autocomplete="off" placeholder="输入 SkillsMP API Key" /></label><button type="button" class="mt-3 inline-flex h-9 items-center gap-2 border border-teal-300 bg-white px-3 text-sm text-teal-800 hover:bg-teal-50" @click="openSkillsMpDocs"><ExternalLink class="size-4" />获取 SkillsMP API Key</button></section>
      <button type="button" class="inline-flex h-10 items-center gap-2 bg-teal-600 px-4 text-sm font-medium text-white disabled:bg-slate-300" :disabled="saving" @click="save"><LoaderCircle v-if="saving" class="size-4 animate-spin" /><Check v-else-if="saved" class="size-4" /><Save v-else class="size-4" />{{ saved ? '已保存' : '保存设置' }}</button>
    </div>
  </section>
</template>
