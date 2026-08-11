<script setup lang="ts">
import { computed, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { CheckCircle2, FolderOpen, LoaderCircle, ScanSearch, ShieldAlert, Sparkles } from "@lucide/vue";
import type { ScanMode, ScanReport } from "../types/skill";

const mode = ref<ScanMode>("fast");
const directory = ref<string | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);
const report = ref<ScanReport | null>(null);

const recommendationClass = computed(() => {
  const recommendation = report.value?.risk_assessment.recommendation;
  return recommendation === "SAFE" ? "text-emerald-700 bg-emerald-50 border-emerald-200" : recommendation === "CAUTION" ? "text-amber-800 bg-amber-50 border-amber-200" : "text-rose-800 bg-rose-50 border-rose-200";
});

async function chooseDirectory() {
  error.value = null;
  const selected = await open({ directory: true, multiple: false, title: "选择完整 skill 文件夹" });
  if (typeof selected === "string") {
    directory.value = selected;
    report.value = null;
  }
}

async function scan() {
  if (!directory.value || loading.value) return;
  loading.value = true;
  error.value = null;
  report.value = null;
  try {
    report.value = await invoke<ScanReport>("scan_skill", { skillPath: directory.value, mode: mode.value });
  } catch (cause) {
    error.value = typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "扫描失败，请重试。";
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <section class="mb-8 border border-slate-200 bg-white p-5 shadow-sm">
    <div class="flex flex-wrap items-start justify-between gap-4">
      <div>
        <div class="flex items-center gap-2 text-sm font-semibold text-slate-950"><ScanSearch class="size-4 text-teal-600" />安全扫描</div>
        <p class="mt-1 text-sm text-slate-500">选择完整 skill 文件夹；扫描会覆盖 SKILL.md、脚本、引用和资源文件。</p>
      </div>
      <div class="inline-flex border border-slate-200 bg-slate-50 p-1 text-xs">
        <button type="button" class="inline-flex h-8 items-center gap-1.5 px-3 font-medium" :class="mode === 'fast' ? 'bg-white text-teal-700 shadow-sm' : 'text-slate-500'" :disabled="loading" @click="mode = 'fast'"><ShieldAlert class="size-3.5" />快速扫描</button>
        <button type="button" class="inline-flex h-8 items-center gap-1.5 px-3 font-medium" :class="mode === 'deep' ? 'bg-white text-teal-700 shadow-sm' : 'text-slate-500'" :disabled="loading" @click="mode = 'deep'"><Sparkles class="size-3.5" />深度扫描</button>
      </div>
    </div>

    <p class="mt-3 text-xs text-slate-500">{{ mode === 'fast' ? '快速扫描：裁剪版静态规则、YARA、行为与 MCP 静态检查。' : '深度扫描：完整 SkillSpector，包含 LLM 语义分析，需要先配置可用的模型凭据。' }}</p>

    <div class="mt-4 flex flex-wrap items-center gap-3">
      <button type="button" class="inline-flex h-10 items-center gap-2 border border-slate-300 bg-white px-3 text-sm font-medium text-slate-700 hover:border-teal-400 hover:text-teal-700" :disabled="loading" @click="chooseDirectory"><FolderOpen class="size-4" />选择文件夹</button>
      <span class="min-w-0 flex-1 truncate text-sm text-slate-500">{{ directory ?? '尚未选择 skill 文件夹' }}</span>
      <button type="button" class="inline-flex h-10 items-center gap-2 bg-teal-600 px-4 text-sm font-medium text-white hover:bg-teal-700 disabled:cursor-not-allowed disabled:bg-slate-300" :disabled="!directory || loading" @click="scan"><LoaderCircle v-if="loading" class="size-4 animate-spin" /><ScanSearch v-else class="size-4" />{{ loading ? '正在扫描' : '开始扫描' }}</button>
    </div>

    <p v-if="error" class="mt-4 border-l-4 border-rose-500 bg-rose-50 px-3 py-2 text-sm text-rose-900" role="alert">{{ error }}</p>

    <div v-if="report" class="mt-5 border-t border-slate-200 pt-5">
      <div class="flex flex-wrap items-center gap-3">
        <div class="flex size-14 items-center justify-center border text-lg font-bold" :class="recommendationClass">{{ report.risk_assessment.score }}</div>
        <div>
          <p class="text-sm font-semibold text-slate-900">{{ report.risk_assessment.severity }} 风险</p>
          <p class="text-xs text-slate-500">{{ report.issues.length }} 个问题。扫描结果仅供决策参考，仍可继续安装。</p>
        </div>
        <span class="border px-2 py-1 text-xs font-semibold" :class="recommendationClass">{{ report.risk_assessment.recommendation.replace(/_/g, ' ') }}</span>
      </div>
      <div v-if="report.issues.length" class="mt-4 divide-y divide-slate-200 border border-slate-200">
        <details v-for="issue in report.issues" :key="`${issue.id}-${issue.location.file}-${issue.location.start_line}`" class="group px-4 py-3">
          <summary class="flex cursor-pointer list-none items-center justify-between gap-3 text-sm"><span class="font-medium text-slate-800">{{ issue.id }} · {{ issue.category ?? 'Security finding' }}</span><span class="shrink-0 text-xs font-semibold" :class="issue.severity === 'HIGH' || issue.severity === 'CRITICAL' ? 'text-rose-700' : issue.severity === 'MEDIUM' ? 'text-amber-700' : 'text-slate-500'">{{ issue.severity }}</span></summary>
          <p class="mt-2 text-sm leading-6 text-slate-600">{{ issue.explanation }}</p>
          <p class="mt-2 text-xs text-slate-500">{{ issue.location.file }}:{{ issue.location.start_line }} · 置信度 {{ Math.round(issue.confidence * 100) }}%</p>
          <p v-if="issue.remediation" class="mt-2 text-sm text-teal-800">修复建议：{{ issue.remediation }}</p>
        </details>
      </div>
      <div v-else class="mt-4 flex items-center gap-2 bg-emerald-50 px-3 py-3 text-sm text-emerald-800"><CheckCircle2 class="size-4" />未发现可报告的问题。</div>
    </div>
  </section>
</template>
