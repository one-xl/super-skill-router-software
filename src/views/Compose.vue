<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { Check, Clipboard, Download, LoaderCircle, Plus, ShieldAlert, Sparkles, Wand2, X } from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";
import { loadInstalledRecords, recordInstallations } from "../lib/database";
import { useSkillIndexStore } from "../stores";
import type { BatchInstallReport, ConversionResult, ConverterSkill, PreparedInstall, ScanReport, Skill, TargetDetection, TargetId } from "../types/skill";

const store = useSkillIndexStore();
const requirement = ref("");
const installed = ref<Skill[]>([]);
const conversion = ref<ConversionResult | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);
const copied = ref(false);
const refining = ref(false);
const manualSelectedIds = ref<string[] | null>(null);
const addSkillId = ref("");
const gapPreparing = ref<string | null>(null);
const gapInstalling = ref(false);
const gapPrepared = ref<{ skill: Skill; prepared: PreparedInstall; targets: TargetDetection[]; target: TargetId } | null>(null);
const gapReport = ref<ScanReport | null>(null);
const gapSkipped = ref(false);
const gapScanning = ref(false);

function toMetadata(skill: Skill): ConverterSkill {
  return { id: skill.id, name: skill.name, description: skill.description, whenToUse: skill.whenToUse, tags: skill.tags, frecency: 0 };
}

function fail(cause: unknown) {
  return typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "操作失败，请重试。";
}

async function refreshInstalled() {
  const records = await loadInstalledRecords();
  const installedKeys = new Set(records.map((record) => `${record.repository}@${record.commit_sha}`));
  installed.value = store.skills.filter((skill) => installedKeys.has(`${skill.repo}@${skill.commit_sha}`));
}

async function convert() {
  if (!requirement.value.trim() || !store.skills.length) {
    conversion.value = null;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    conversion.value = await invoke<ConversionResult>("convert_requirement", {
      request: {
        requirement: requirement.value,
        installed: installed.value.map(toMetadata),
        index: store.skills.map(toMetadata),
        selectedIds: manualSelectedIds.value,
      },
    });
  } catch (cause) {
    error.value = fail(cause);
  } finally {
    loading.value = false;
  }
}

let timer: number | undefined;
watch([requirement, () => installed.value, () => store.skills], () => {
  window.clearTimeout(timer);
  timer = window.setTimeout(() => { void convert(); }, 180);
}, { deep: true });

const selectedIds = computed(() => new Set(conversion.value?.selected.map((skill) => skill.id) ?? []));
const selectableInstalled = computed(() => installed.value.filter((skill) => !selectedIds.value.has(skill.id)));
const canAddSkill = computed(() => selectedIds.value.size < 5);

function ensureManualSelection() {
  if (manualSelectedIds.value === null) manualSelectedIds.value = conversion.value?.selected.map((skill) => skill.id) ?? [];
}

function removeSkill(id: string) {
  ensureManualSelection();
  manualSelectedIds.value = manualSelectedIds.value?.filter((selected) => selected !== id) ?? [];
  void convert();
}

function addSkill() {
  if (!addSkillId.value) return;
  ensureManualSelection();
  if (!manualSelectedIds.value?.includes(addSkillId.value)) manualSelectedIds.value?.push(addSkillId.value);
  addSkillId.value = "";
  void convert();
}

async function copyPrompt() {
  if (!conversion.value) return;
  try {
    await navigator.clipboard.writeText(conversion.value.prompt);
    copied.value = true;
    window.setTimeout(() => { copied.value = false; }, 1500);
  } catch (cause) {
    error.value = `无法复制 prompt：${fail(cause)}`;
  }
}
async function refinePrompt() {
  if (!conversion.value || refining.value) return;
  refining.value = true; error.value = null;
  try { conversion.value = { ...conversion.value, prompt: await invoke<string>("refine_prompt", { request: { requirement: requirement.value, templatePrompt: conversion.value.prompt } }) }; }
  catch (cause) { error.value = fail(cause); } finally { refining.value = false; }
}

async function prepareGapInstall(skillId: string) {
  const skill = store.skills.find((candidate) => candidate.id === skillId);
  if (!skill) return;
  gapPreparing.value = skillId;
  error.value = null;
  try {
    const [prepared, targets] = await Promise.all([
      invoke<PreparedInstall>("prepare_skill_install", { skill }),
      invoke<TargetDetection[]>("detect_skill_targets"),
    ]);
    const target = targets.find((candidate) => candidate.id === "claude_code" && candidate.available)?.id ?? targets.find((candidate) => candidate.id !== "claude_desktop" && candidate.available)?.id;
    if (!target) throw new Error("未检测到可自动部署的目标，请先安装 Claude Code 或 Codex CLI。");
    gapPrepared.value = { skill, prepared, targets, target };
    gapReport.value = null; gapSkipped.value = false;
  } catch (cause) {
    error.value = fail(cause);
  } finally {
    gapPreparing.value = null;
  }
}

async function scanGap(mode: "fast" | "deep" | "skip") {
  if (!gapPrepared.value || gapScanning.value) return;
  if (mode === "skip") { gapSkipped.value = true; return; }
  gapScanning.value = true; error.value = null;
  try { gapReport.value = await invoke<ScanReport>("scan_prepared_skill", { token: gapPrepared.value.prepared.token, mode }); }
  catch (cause) { error.value = fail(cause); } finally { gapScanning.value = false; }
}

async function installGap() {
  if (!gapPrepared.value || gapInstalling.value) return;
  gapInstalling.value = true;
  error.value = null;
  try {
    const report = await invoke<BatchInstallReport>("install_prepared_skill", { token: gapPrepared.value.prepared.token, targets: [gapPrepared.value.target] });
    if (!report.results.some((result) => result.outcome?.kind === "installed")) throw new Error(report.results.map((result) => result.error).filter(Boolean).join("；") || "skill 未能安装。");
    await recordInstallations(gapPrepared.value.skill, gapPrepared.value.prepared.directory_name, report, gapPrepared.value.prepared.commit_sha);
    const installedId = gapPrepared.value.skill.id;
    gapPrepared.value = null;
    await refreshInstalled();
    ensureManualSelection();
    if (!manualSelectedIds.value?.includes(installedId)) manualSelectedIds.value?.push(installedId);
    await convert();
  } catch (cause) {
    error.value = fail(cause);
  } finally {
    gapInstalling.value = false;
  }
}

onMounted(async () => {
  if (!store.index) await store.load();
  try { await refreshInstalled(); } catch (cause) { error.value = `无法读取本机安装记录：${fail(cause)}`; }
});
</script>

<template>
  <section class="page-shell">
    <div class="page-header">
      <div>
        <p class="page-kicker">Compose</p>
        <h1 class="page-title">需求转 Prompt</h1>
        <p class="page-description">匹配已安装的相关 skill，生成可直接执行的结构化 Prompt。</p>
      </div>
    </div>

    <p v-if="error" class="notice-error mb-5" role="alert">{{ error }}</p>

    <div class="grid items-start gap-5 lg:grid-cols-2">
      <section class="surface overflow-hidden">
        <div class="border-b border-stone-200 px-5 py-4">
          <label class="section-title" for="requirement">需求</label>
          <p class="mt-1 text-[11px] text-stone-500">说明目标、技术栈、限制与完成标准。</p>
        </div>
        <div class="p-5">
          <textarea id="requirement" v-model="requirement" class="textarea-field min-h-72 resize-y" placeholder="描述你希望 agent 完成的工作、技术栈和限制..." />

          <div v-if="conversion" class="mt-5 border-t border-stone-200 pt-5">
            <div class="flex items-center justify-between gap-3"><h2 class="section-title">已选 Skill</h2><span class="rounded-full bg-stone-100 px-2.5 py-1 text-[10px] font-medium text-stone-500">{{ conversion.scenario }}</span></div>
            <div class="mt-3 flex flex-wrap gap-2">
              <span v-for="skill in conversion.selected" :key="skill.id" class="inline-flex items-center gap-1.5 rounded-full border border-teal-200 bg-teal-50 px-2.5 py-1 text-[11px] font-medium text-teal-800">{{ skill.name }}<button type="button" class="rounded-full text-teal-600 transition hover:text-rose-700" :title="`移除 ${skill.name}`" @click="removeSkill(skill.id)"><X class="size-3" /></button></span>
              <span v-if="!conversion.selected.length" class="text-[12px] text-stone-500">未推荐已安装 skill。</span>
            </div>

            <div v-if="selectableInstalled.length" class="mt-3 flex items-center gap-2">
              <select v-model="addSkillId" class="select-field h-9 min-w-0 flex-1 text-[12px]" :disabled="!canAddSkill"><option value="">手动添加已安装 skill</option><option v-for="skill in selectableInstalled" :key="skill.id" :value="skill.id">{{ skill.name }}</option></select>
              <button type="button" class="icon-button" :disabled="!canAddSkill" title="添加 skill" @click="addSkill"><Plus class="size-4" /></button>
            </div>

            <div v-if="conversion.gaps.length" class="mt-5 border-t border-stone-200 pt-4">
              <h2 class="section-title">相关但未安装</h2>
              <div class="mt-3 flex flex-wrap gap-2">
                <span v-for="skill in conversion.gaps" :key="skill.id" class="inline-flex items-center gap-2 rounded-full border border-stone-200 bg-stone-100 px-2.5 py-1 text-[11px] text-stone-600">{{ skill.name }}<button type="button" class="inline-flex items-center gap-1 font-medium text-teal-700 transition hover:text-teal-900 disabled:opacity-50" :disabled="gapPreparing === skill.id" @click="prepareGapInstall(skill.id)"><LoaderCircle v-if="gapPreparing === skill.id" class="size-3 animate-spin" /><Download v-else class="size-3" />安装</button></span>
              </div>
            </div>
          </div>

          <div v-if="gapPrepared" class="notice-warning mt-5">
            <div class="flex items-center gap-2 font-semibold"><ShieldAlert class="size-4" />{{ gapReport ? `扫描完成：${gapReport.risk_assessment.score}/100 · ${gapReport.risk_assessment.recommendation.replace(/_/g, ' ')}` : gapSkipped ? '已跳过扫描' : '选择安装前扫描方式' }}</div>
            <div v-if="!gapReport && !gapSkipped" class="mt-3 flex flex-wrap gap-2"><button type="button" class="button-ghost border border-amber-300 bg-white" :disabled="gapScanning" @click="scanGap('skip')">跳过扫描</button><button type="button" class="button-primary" :disabled="gapScanning" @click="scanGap('fast')"><LoaderCircle v-if="gapScanning" class="size-4 animate-spin" />快速扫描</button><button type="button" class="button-secondary border-amber-300" :disabled="gapScanning" @click="scanGap('deep')"><Sparkles class="size-4" />深度扫描</button></div>
            <div v-else class="mt-3 flex flex-wrap items-center gap-2"><select v-model="gapPrepared.target" class="select-field h-9 min-w-44 flex-1"><option v-for="target in gapPrepared.targets.filter((target) => target.id !== 'claude_desktop' && target.available)" :key="target.id" :value="target.id">{{ target.name }}</option></select><button type="button" class="button-primary" :disabled="gapInstalling" @click="installGap"><LoaderCircle v-if="gapInstalling" class="size-4 animate-spin" />{{ gapInstalling ? '正在安装' : '安装并更新 Prompt' }}</button></div>
            <p class="mt-2 text-[11px]">扫描仅辅助判断，是否继续安装由你决定。</p>
          </div>
        </div>
      </section>

      <section class="surface overflow-hidden lg:sticky lg:top-[68px]">
        <div class="flex flex-wrap items-center justify-between gap-3 border-b border-stone-200 px-5 py-3.5">
          <div><h2 class="section-title">实时预览</h2><p class="mt-1 text-[11px] text-stone-500">{{ loading ? '正在匹配 Skill' : conversion ? '已生成' : '等待输入' }}</p></div>
          <div class="flex gap-2"><button type="button" class="button-secondary" :disabled="!conversion || refining" @click="refinePrompt"><LoaderCircle v-if="refining" class="size-4 animate-spin" /><Wand2 v-else class="size-4" />LLM 精炼</button><button type="button" class="button-secondary" :disabled="!conversion" @click="copyPrompt"><Check v-if="copied" class="size-4 text-emerald-600" /><Clipboard v-else class="size-4" />{{ copied ? '已复制' : '复制' }}</button></div>
        </div>
        <pre class="min-h-[34rem] whitespace-pre-wrap bg-stone-950 p-5 font-mono text-[12px] leading-6 text-stone-200">{{ loading ? '正在匹配已安装 skill...' : conversion?.prompt || '输入需求后将在此生成结构化 prompt。' }}</pre>
      </section>
    </div>
  </section>
</template>
