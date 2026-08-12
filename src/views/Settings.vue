<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Bot, Check, CircleAlert, CircleCheck, ExternalLink, KeyRound, LoaderCircle, Save, Settings2 } from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { ApiConfig, AppSettings } from "../types/skill";

const SKILLSMP_DOCS_URL = "https://skillsmp.com/zh/docs/api";
const empty = (): ApiConfig => ({ format: "openai", apiUrl: "", apiKey: "", model: "", apiKeyConfigured: false });
const settings = ref<AppSettings>({
  deepScan: empty(),
  prompt: empty(),
  skillsMp: { apiKey: "", apiKeyConfigured: false },
  automation: { autoInjectAfterRefine: false, startCodexRecoveryMonitorOnLaunch: false },
});
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
  <section class="page-shell max-w-[980px]">
    <div class="page-header">
      <div>
        <p class="page-kicker">Settings</p>
        <h1 class="page-title">模型与远程源</h1>
        <p class="page-description">配置均为可选；密钥由 Windows Credential Manager 安全保存。</p>
      </div>
      <button v-if="!loading" type="button" class="button-primary" :disabled="saving" @click="save">
        <LoaderCircle v-if="saving" class="size-4 animate-spin" />
        <Check v-else-if="saved" class="size-4" />
        <Save v-else class="size-4" />
        {{ saved ? '已保存' : '保存设置' }}
      </button>
    </div>

    <p v-if="error" class="notice-error mb-4">{{ error }}</p>
    <div v-if="loading" class="surface flex items-center gap-2 p-5 text-[13px] text-stone-500"><LoaderCircle class="size-4 animate-spin" />正在读取设置</div>

    <div v-else class="space-y-5">
      <div class="surface grid overflow-hidden sm:grid-cols-3">
        <div v-for="item in configurationSummary" :key="item.label" class="flex items-center justify-between gap-3 border-b border-stone-200 p-4 last:border-b-0 sm:border-b-0 sm:border-r sm:last:border-r-0">
          <div>
            <p class="text-[13px] font-medium text-stone-800">{{ item.label }}</p>
            <p class="mt-1 text-[11px] text-stone-500">{{ item.suggested ? '建议配置' : '按需配置' }}</p>
          </div>
          <span class="inline-flex shrink-0 items-center gap-1 text-[11px] font-medium" :class="item.configured ? 'text-emerald-700' : 'text-stone-400'">
            <CircleCheck v-if="item.configured" class="size-4" /><CircleAlert v-else class="size-4" />{{ item.configured ? '已配置' : '未配置' }}
          </span>
        </div>
      </div>

      <section v-for="section in apiSections" :key="section.key" class="surface p-5">
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h2 class="section-title flex items-center gap-2"><Settings2 class="size-4 text-teal-700" />{{ section.title }}</h2>
            <p class="mt-1.5 text-[12px] text-stone-500">{{ section.hint }}</p>
          </div>
          <span class="inline-flex items-center gap-1 rounded-full border px-2.5 py-1 text-[11px] font-medium" :class="apiConfigured(settings[section.key]) ? 'border-emerald-200 bg-emerald-50 text-emerald-700' : 'border-stone-200 bg-stone-50 text-stone-500'">
            <CircleCheck v-if="apiConfigured(settings[section.key])" class="size-3.5" /><CircleAlert v-else class="size-3.5" />{{ apiConfigured(settings[section.key]) ? '已配置' : '未配置' }}
          </span>
        </div>
        <div class="mt-5 grid gap-4 sm:grid-cols-2">
          <label class="field-label">接口格式<select v-model="settings[section.key].format" class="select-field mt-1.5"><option value="openai">OpenAI 兼容</option><option value="anthropic">Anthropic Messages</option></select></label>
          <label class="field-label">模型（可选）<input v-model="settings[section.key].model" class="field mt-1.5" placeholder="例如 gpt-4o-mini" /></label>
          <label class="field-label sm:col-span-2">API URL<input v-model="settings[section.key].apiUrl" class="field mt-1.5" placeholder="https://api.openai.com/v1" /></label>
          <label class="field-label sm:col-span-2">API Key <span v-if="settings[section.key].apiKeyConfigured" class="ml-1 text-[11px] font-normal text-emerald-700">已安全保存，留空则保持不变</span><input v-model="settings[section.key].apiKey" type="password" class="field mt-1.5" autocomplete="off" placeholder="输入新密钥以替换" /></label>
        </div>
      </section>

      <section class="surface p-5">
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h2 class="section-title flex items-center gap-2"><KeyRound class="size-4 text-teal-700" />SkillsMP 远程发现</h2>
            <p class="mt-1.5 text-[12px] text-stone-500">用于主动远程搜索；下载仍会锁定 commit 并提取完整 skill 目录。</p>
          </div>
          <span class="inline-flex items-center gap-1 rounded-full border px-2.5 py-1 text-[11px] font-medium" :class="settings.skillsMp.apiKeyConfigured ? 'border-emerald-200 bg-emerald-50 text-emerald-700' : 'border-amber-200 bg-amber-50 text-amber-700'">
            <CircleCheck v-if="settings.skillsMp.apiKeyConfigured" class="size-3.5" /><CircleAlert v-else class="size-3.5" />{{ settings.skillsMp.apiKeyConfigured ? '已配置' : '建议配置' }}
          </span>
        </div>
        <label class="field-label mt-5">SkillsMP API Key <span v-if="settings.skillsMp.apiKeyConfigured" class="ml-1 text-[11px] font-normal text-emerald-700">已安全保存，留空则保持不变</span><input v-model="settings.skillsMp.apiKey" type="password" class="field mt-1.5" autocomplete="off" placeholder="输入 SkillsMP API Key" /></label>
        <button type="button" class="button-secondary mt-3" @click="openSkillsMpDocs"><ExternalLink class="size-4" />获取 SkillsMP API Key</button>
      </section>

      <section class="surface p-5">
        <div>
          <h2 class="section-title flex items-center gap-2"><Bot class="size-4 text-teal-700" />桌面 Agent 自动化</h2>
          <p class="mt-1.5 text-[12px] text-stone-500">自动行为默认关闭，可单独启用；手动填入和手动开始监控始终可用。</p>
        </div>
        <div class="mt-5 divide-y divide-stone-200 border-y border-stone-200">
          <label class="flex cursor-pointer items-start justify-between gap-5 py-4">
            <span>
              <span class="block text-[13px] font-medium text-stone-800">LLM 精炼后自动填入桌面 Agent</span>
              <span class="mt-1 block text-[11px] leading-5 text-stone-500">将精炼结果填入所选 Codex Desktop 或 Claude Code Desktop 对话框，但不会自动发送。</span>
            </span>
            <input v-model="settings.automation.autoInjectAfterRefine" type="checkbox" class="mt-0.5 size-4 shrink-0 accent-teal-700" />
          </label>
          <label class="flex cursor-pointer items-start justify-between gap-5 py-4">
            <span>
              <span class="block text-[13px] font-medium text-stone-800">启动软件时监控 Codex Desktop 重连</span>
              <span class="mt-1 block text-[11px] leading-5 text-stone-500">仅在 Codex 日志确认第 5 次重连失败后，自动输入并发送“继续并恢复todo-list”。</span>
            </span>
            <input v-model="settings.automation.startCodexRecoveryMonitorOnLaunch" type="checkbox" class="mt-0.5 size-4 shrink-0 accent-teal-700" />
          </label>
        </div>
      </section>
    </div>
  </section>
</template>
