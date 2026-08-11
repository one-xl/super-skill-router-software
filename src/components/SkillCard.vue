<script setup lang="ts">
import { computed, ref } from "vue";
import { CheckCircle2, Download, ExternalLink, FileText, GitBranch, LoaderCircle, ShieldAlert, Sparkles } from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";
import { recordInstallations } from "../lib/database";
import UploadGuide from "./UploadGuide.vue";
import type { BatchInstallReport, PreparedInstall, ScanMode, ScanReport, SkillSearchResult, TargetDetection, TargetId } from "../types/skill";

const props = defineProps<{ result: SkillSearchResult }>();
const preparing = ref(false);
const installing = ref(false);
const error = ref<string | null>(null);
const prepared = ref<PreparedInstall | null>(null);
const report = ref<ScanReport | null>(null);
const scanning = ref(false);
const scanSkipped = ref(false);
const targets = ref<TargetDetection[]>([]);
const selectedTargets = ref<TargetId[]>([]);
const deployment = ref<BatchInstallReport | null>(null);
const uploadPackages = computed(() => deployment.value?.results.flatMap((result) => result.outcome?.kind === "packaged_for_upload" && result.outcome.zip_path ? [{ targetName: result.target_name, zipPath: result.outcome.zip_path }] : []) ?? []);

function failureMessage(cause: unknown) {
  return typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "操作失败，请重试。";
}

async function prepareInstall() {
  if (preparing.value) return;
  preparing.value = true;
  error.value = null;
  deployment.value = null;
  try {
    const [nextPrepared, detections] = await Promise.all([
      invoke<PreparedInstall>("prepare_skill_install", { skill: props.result.skill }),
      invoke<TargetDetection[]>("detect_skill_targets"),
    ]);
    prepared.value = nextPrepared;
    targets.value = detections;
    selectedTargets.value = detections.filter((target) => target.available && target.id !== "claude_desktop").map((target) => target.id);
  } catch (cause) {
    error.value = failureMessage(cause);
  } finally {
    preparing.value = false;
  }
}

async function scan(mode: ScanMode | "skip") {
  if (!prepared.value || scanning.value) return;
  if (mode === "skip") { scanSkipped.value = true; return; }
  scanning.value = true; error.value = null;
  try { report.value = await invoke<ScanReport>("scan_prepared_skill", { token: prepared.value.token, mode }); }
  catch (cause) { error.value = failureMessage(cause); } finally { scanning.value = false; }
}

async function installSelectedTargets() {
  if (!prepared.value || installing.value) return;
  if (selectedTargets.value.length === 0) {
    error.value = "请至少选择一个已探测到的自动部署目标。";
    return;
  }
  installing.value = true;
  error.value = null;
  try {
    const report = await invoke<BatchInstallReport>("install_prepared_skill", { token: prepared.value.token, targets: selectedTargets.value });
    deployment.value = report;
    try {
      await recordInstallations(props.result.skill, prepared.value.directory_name, report, prepared.value.commit_sha);
    } catch (databaseError) {
      error.value = `文件已部署，但安装记录写入失败：${failureMessage(databaseError)}`;
    }
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
          <span class="inline-flex items-center gap-1.5"><FileText class="size-3.5" />{{ result.skill.remote_source === 'skillsmp' ? '完整目录将在下载时解析' : `${result.skill.files.length} 个文件` }}</span>
          <span>{{ result.skill.remote_source === 'skillsmp' ? '下载时锁定 commit' : `固定版本 ${result.skill.commit_sha.slice(0, 8)}` }}</span>
          <span v-for="tag in result.skill.tags" :key="tag" class="rounded bg-slate-100 px-2 py-0.5 text-slate-500">#{{ tag }}</span>
        </div>
      </div>
      <div class="flex shrink-0 items-center gap-2">
        <button class="flex size-9 items-center justify-center rounded-lg border border-teal-600 bg-teal-600 text-white transition hover:bg-teal-700 disabled:cursor-not-allowed disabled:border-slate-300 disabled:bg-slate-300" type="button" :disabled="preparing || installing || !!deployment" title="下载、扫描并选择部署目标" @click="prepareInstall">
          <LoaderCircle v-if="preparing" class="size-4 animate-spin" />
          <Download v-else class="size-4" :stroke-width="1.8" />
        </button>
        <a class="flex size-9 items-center justify-center rounded-lg border border-slate-200 text-slate-400 transition hover:border-teal-300 hover:bg-teal-50 hover:text-teal-700" :href="result.skill.source.rawUrl" target="_blank" rel="noreferrer" title="在 GitHub 查看 SKILL.md" aria-label="在 GitHub 查看 SKILL.md"><ExternalLink class="size-4" :stroke-width="1.8" /></a>
      </div>
    </div>
    <div v-if="error && !deployment" class="mt-4 border-l-4 border-rose-500 bg-rose-50 px-3 py-2 text-sm text-rose-900" role="alert">{{ error }}</div>
    <div v-else-if="deployment" class="mt-4 border-l-4 border-emerald-500 bg-emerald-50 px-3 py-2 text-sm text-emerald-900">
      <div class="flex items-center gap-2"><CheckCircle2 class="size-4" />部署完成</div>
      <ul class="mt-2 space-y-1 text-xs">
        <li v-for="result in deployment.results" :key="result.target">{{ result.target_name }}：{{ result.outcome?.kind === 'packaged_for_upload' ? '已打包，待上传' : result.outcome ? `已部署${result.reused_physical_install ? '（复用共享目录）' : ''}` : result.error }}</li>
      </ul>
      <UploadGuide v-if="uploadPackages.length" :packages="uploadPackages" />
      <p v-if="error" class="mt-2 border-t border-rose-200 pt-2 text-xs text-rose-800">{{ error }}</p>
    </div>
    <div v-else-if="prepared && !report && !scanSkipped" class="mt-4 border border-slate-200 bg-slate-50 p-4">
      <div class="flex items-center gap-2 text-sm font-semibold text-slate-900"><ShieldAlert class="size-4 text-teal-700" />下载完成，选择安装前扫描方式</div>
      <div class="mt-3 flex flex-wrap gap-2"><button type="button" class="h-9 border border-slate-300 bg-white px-3 text-sm text-slate-700" :disabled="scanning" @click="scan('skip')">跳过扫描</button><button type="button" class="h-9 bg-teal-600 px-3 text-sm text-white disabled:bg-slate-300" :disabled="scanning" @click="scan('fast')"><LoaderCircle v-if="scanning" class="mr-1 inline size-4 animate-spin" />快速扫描</button><button type="button" class="inline-flex h-9 items-center gap-1 border border-teal-300 bg-white px-3 text-sm text-teal-800 disabled:opacity-50" :disabled="scanning" @click="scan('deep')"><Sparkles class="size-4" />深度扫描</button></div>
      <p class="mt-2 text-xs text-slate-500">深度扫描使用设置页中的模型配置；扫描只作决策提示，仍由你决定是否部署。</p>
    </div>
    <div v-else-if="prepared" class="mt-4 border border-slate-200 bg-slate-50 p-3">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <p class="inline-flex items-center gap-2 text-sm font-medium text-slate-800"><ShieldAlert class="size-4" :class="report?.risk_assessment.recommendation === 'SAFE' ? 'text-emerald-600' : 'text-rose-600'" />{{ scanSkipped ? '已跳过扫描' : `扫描完成：${report?.risk_assessment.score}/100 · ${report?.risk_assessment.recommendation.replace(/_/g, ' ')}` }}</p>
        <button class="inline-flex h-9 items-center gap-2 bg-teal-600 px-3 text-sm font-medium text-white hover:bg-teal-700 disabled:cursor-not-allowed disabled:bg-slate-300" type="button" :disabled="installing || selectedTargets.length === 0" @click="installSelectedTargets"><LoaderCircle v-if="installing" class="size-4 animate-spin" />{{ installing ? '正在部署' : '部署到所选目标' }}</button>
      </div>
      <fieldset class="mt-3 grid gap-2 sm:grid-cols-2">
        <label v-for="target in targets" :key="target.id" class="flex items-center justify-between gap-3 border border-slate-200 bg-white px-3 py-2 text-sm" :class="target.id === 'claude_desktop' ? 'border-amber-200' : ''">
          <span class="flex items-center gap-2"><input v-model="selectedTargets" type="checkbox" :value="target.id" :disabled="target.id !== 'claude_desktop' && !target.available" />{{ target.name }}</span>
          <span class="text-xs text-slate-500">{{ target.id === 'claude_desktop' ? '打包后待上传' : target.available ? '已探测' : '未检测到' }}</span>
        </label>
      </fieldset>
      <p class="mt-2 text-xs text-slate-500">扫描结果用于辅助决策；即使存在高风险提示，仍由你决定是否继续安装。</p>
    </div>
  </article>
</template>
