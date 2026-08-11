<script setup lang="ts">
import { ref } from "vue";
import { CheckCircle2, Download, ExternalLink, FileText, GitBranch, LoaderCircle, ShieldAlert } from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";
import type { InstallOutcome, PreparedInstall, SkillSearchResult } from "../types/skill";

const props = defineProps<{ result: SkillSearchResult }>();
const preparing = ref(false);
const installing = ref(false);
const error = ref<string | null>(null);
const prepared = ref<PreparedInstall | null>(null);
const installedPath = ref<string | null>(null);

function failureMessage(cause: unknown) {
  return typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "操作失败，请重试。";
}

async function prepareInstall() {
  if (preparing.value) return;
  preparing.value = true;
  error.value = null;
  installedPath.value = null;
  try {
    prepared.value = await invoke<PreparedInstall>("prepare_claude_code_install", { skill: props.result.skill });
  } catch (cause) {
    error.value = failureMessage(cause);
  } finally {
    preparing.value = false;
  }
}

async function installToClaudeCode() {
  if (!prepared.value || installing.value) return;
  installing.value = true;
  error.value = null;
  try {
    const outcome = await invoke<InstallOutcome>("install_prepared_claude_code", { token: prepared.value.token });
    installedPath.value = outcome.path ?? null;
  } catch (cause) {
    error.value = failureMessage(cause);
  } finally {
    installing.value = false;
  }
}
</script>

<template>
  <article class="group border-b border-slate-200 py-6 first:pt-2 last:border-b-0">
    <div class="flex items-start justify-between gap-5">
      <div class="min-w-0">
        <div class="mb-2 flex flex-wrap items-center gap-2">
          <h2 class="truncate text-lg font-semibold text-slate-950">{{ result.skill.name }}</h2>
          <span v-for="field in result.matchedFields" :key="field" class="rounded-full bg-teal-50 px-2 py-0.5 text-[11px] font-medium text-teal-700">
            命中{{ field === 'name' ? '名称' : field === 'whenToUse' ? '触发场景' : '描述' }}
          </span>
        </div>
        <p class="mb-3 max-w-3xl text-sm leading-6 text-slate-600">{{ result.skill.description }}</p>
        <p v-if="result.skill.whenToUse" class="mb-3 max-w-3xl text-sm leading-6 text-slate-500">
          <span class="font-medium text-slate-700">适用场景：</span>{{ result.skill.whenToUse }}
        </p>
        <div class="flex flex-wrap items-center gap-x-4 gap-y-2 text-xs text-slate-400">
          <span class="inline-flex items-center gap-1.5"><GitBranch class="size-3.5" />{{ result.skill.repo }}</span>
          <span class="inline-flex items-center gap-1.5"><FileText class="size-3.5" />{{ result.skill.files.length }} 个文件</span>
          <span>固定版本 {{ result.skill.commit_sha.slice(0, 8) }}</span>
          <span v-for="tag in result.skill.tags" :key="tag" class="rounded bg-slate-100 px-2 py-0.5 text-slate-500">#{{ tag }}</span>
        </div>
      </div>
      <div class="flex shrink-0 items-center gap-2">
        <button class="flex size-9 items-center justify-center rounded-lg border border-teal-600 bg-teal-600 text-white transition hover:bg-teal-700 disabled:cursor-not-allowed disabled:border-slate-300 disabled:bg-slate-300" type="button" :disabled="preparing || installing || !!installedPath" title="下载、扫描并准备安装到 Claude Code" @click="prepareInstall">
          <LoaderCircle v-if="preparing" class="size-4 animate-spin" />
          <Download v-else class="size-4" :stroke-width="1.8" />
        </button>
        <a class="flex size-9 items-center justify-center rounded-lg border border-slate-200 text-slate-400 transition hover:border-teal-300 hover:bg-teal-50 hover:text-teal-700" :href="result.skill.source.rawUrl" target="_blank" rel="noreferrer" title="在 GitHub 查看 SKILL.md" aria-label="在 GitHub 查看 SKILL.md"><ExternalLink class="size-4" :stroke-width="1.8" /></a>
      </div>
    </div>
    <div v-if="error" class="mt-4 border-l-4 border-rose-500 bg-rose-50 px-3 py-2 text-sm text-rose-900" role="alert">{{ error }}</div>
    <div v-else-if="installedPath" class="mt-4 flex items-center gap-2 border-l-4 border-emerald-500 bg-emerald-50 px-3 py-2 text-sm text-emerald-900"><CheckCircle2 class="size-4" />已安装到 Claude Code：{{ installedPath }}</div>
    <div v-else-if="prepared" class="mt-4 border border-slate-200 bg-slate-50 p-3">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <p class="inline-flex items-center gap-2 text-sm font-medium text-slate-800"><ShieldAlert class="size-4" :class="prepared.report.risk_assessment.recommendation === 'SAFE' ? 'text-emerald-600' : 'text-rose-600'" />扫描完成：{{ prepared.report.risk_assessment.score }}/100 · {{ prepared.report.risk_assessment.recommendation.replace(/_/g, ' ') }}</p>
        <button class="inline-flex h-9 items-center gap-2 bg-teal-600 px-3 text-sm font-medium text-white hover:bg-teal-700 disabled:cursor-not-allowed disabled:bg-slate-300" type="button" :disabled="installing" @click="installToClaudeCode"><LoaderCircle v-if="installing" class="size-4 animate-spin" />{{ installing ? '正在安装' : '继续安装到 Claude Code' }}</button>
      </div>
      <p class="mt-2 text-xs text-slate-500">扫描结果用于辅助决策；即使存在高风险提示，仍由你决定是否继续安装。</p>
    </div>
  </article>
</template>
