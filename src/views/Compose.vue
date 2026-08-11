<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { Check, Clipboard, Download, LoaderCircle, Plus, ShieldAlert, X } from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";
import { loadInstalledRecords, recordInstallations } from "../lib/database";
import { useSkillIndexStore } from "../stores";
import type { BatchInstallReport, ConversionResult, ConverterSkill, PreparedInstall, Skill, TargetDetection, TargetId } from "../types/skill";

const store = useSkillIndexStore();
const requirement = ref("");
const installed = ref<Skill[]>([]);
const conversion = ref<ConversionResult | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);
const copied = ref(false);
const manualSelectedIds = ref<string[] | null>(null);
const addSkillId = ref("");
const gapPreparing = ref<string | null>(null);
const gapInstalling = ref(false);
const gapPrepared = ref<{ skill: Skill; prepared: PreparedInstall; targets: TargetDetection[]; target: TargetId } | null>(null);

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
  } catch (cause) {
    error.value = fail(cause);
  } finally {
    gapPreparing.value = null;
  }
}

async function installGap() {
  if (!gapPrepared.value || gapInstalling.value) return;
  gapInstalling.value = true;
  error.value = null;
  try {
    const report = await invoke<BatchInstallReport>("install_prepared_skill", { token: gapPrepared.value.prepared.token, targets: [gapPrepared.value.target] });
    if (!report.results.some((result) => result.outcome?.kind === "installed")) throw new Error(report.results.map((result) => result.error).filter(Boolean).join("；") || "skill 未能安装。");
    await recordInstallations(gapPrepared.value.skill, gapPrepared.value.prepared.directory_name, report);
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
  <section class="mx-auto w-full max-w-6xl px-6 py-8 lg:px-10">
    <div class="mb-7">
      <p class="mb-2 text-xs font-semibold uppercase tracking-[0.18em] text-teal-700">Compose</p>
      <h1 class="text-3xl font-semibold text-slate-950">需求转 Prompt</h1>
      <p class="mt-2 text-sm text-slate-500">模板模式仅推荐相关且已安装的 skill，不会嵌入 skill 正文。</p>
    </div>
    <p v-if="error" class="mb-5 border-l-4 border-rose-500 bg-rose-50 px-3 py-2 text-sm text-rose-900" role="alert">{{ error }}</p>
    <div class="grid gap-6 lg:grid-cols-2">
      <div>
        <label class="text-sm font-semibold text-slate-800" for="requirement">需求</label>
        <textarea id="requirement" v-model="requirement" class="mt-2 min-h-80 w-full resize-y border border-slate-300 bg-white p-4 text-sm leading-6 outline-none focus:border-teal-600 focus:ring-2 focus:ring-teal-100" placeholder="描述你希望 agent 完成的工作、技术栈和限制..." />
        <div v-if="conversion" class="mt-4 border border-slate-200 bg-white p-4">
          <div class="flex items-center justify-between gap-3"><h2 class="text-sm font-semibold text-slate-900">已选 Skill</h2><span class="text-xs text-slate-500">{{ conversion.scenario }}</span></div>
          <div class="mt-3 flex flex-wrap gap-2">
            <span v-for="skill in conversion.selected" :key="skill.id" class="inline-flex items-center gap-1 border border-teal-200 bg-teal-50 px-2 py-1 text-xs text-teal-800">{{ skill.name }}<button type="button" class="text-teal-700 hover:text-rose-700" :title="`移除 ${skill.name}`" @click="removeSkill(skill.id)"><X class="size-3" /></button></span>
            <span v-if="!conversion.selected.length" class="text-xs text-slate-500">未推荐已安装 skill。</span>
          </div>
          <div v-if="selectableInstalled.length" class="mt-3 flex items-center gap-2"><select v-model="addSkillId" class="h-8 min-w-0 flex-1 border border-slate-300 bg-white px-2 text-xs disabled:cursor-not-allowed disabled:bg-slate-100" :disabled="!canAddSkill"><option value="">手动添加已安装 skill</option><option v-for="skill in selectableInstalled" :key="skill.id" :value="skill.id">{{ skill.name }}</option></select><button type="button" class="flex size-8 items-center justify-center border border-slate-300 text-slate-600 hover:border-teal-500 hover:text-teal-700 disabled:cursor-not-allowed disabled:border-slate-200 disabled:text-slate-300" :disabled="!canAddSkill" title="添加 skill" @click="addSkill"><Plus class="size-4" /></button></div>
          <div v-if="conversion.gaps.length" class="mt-5 border-t border-slate-200 pt-4"><h2 class="text-sm font-semibold text-slate-900">相关但未安装</h2><div class="mt-3 flex flex-wrap gap-2"><span v-for="skill in conversion.gaps" :key="skill.id" class="inline-flex items-center gap-2 border border-slate-300 bg-slate-100 px-2 py-1 text-xs text-slate-600">{{ skill.name }}<button type="button" class="inline-flex items-center gap-1 border border-slate-300 bg-white px-1.5 py-0.5 text-slate-700 hover:border-teal-500 hover:text-teal-700 disabled:opacity-50" :disabled="gapPreparing === skill.id" @click="prepareGapInstall(skill.id)"><LoaderCircle v-if="gapPreparing === skill.id" class="size-3 animate-spin" /><Download v-else class="size-3" />安装</button></span></div></div>
        </div>
        <div v-if="gapPrepared" class="mt-4 border border-amber-300 bg-amber-50 p-4"><div class="flex items-center gap-2 text-sm font-semibold text-amber-950"><ShieldAlert class="size-4" />安装前扫描：{{ gapPrepared.prepared.report.risk_assessment.score }}/100 · {{ gapPrepared.prepared.report.risk_assessment.recommendation.replace(/_/g, ' ') }}</div><div class="mt-3 flex flex-wrap items-center gap-3"><select v-model="gapPrepared.target" class="h-9 border border-amber-300 bg-white px-2 text-sm"><option v-for="target in gapPrepared.targets.filter((target) => target.id !== 'claude_desktop' && target.available)" :key="target.id" :value="target.id">{{ target.name }}</option></select><button type="button" class="inline-flex h-9 items-center gap-2 bg-teal-600 px-3 text-sm font-medium text-white hover:bg-teal-700 disabled:bg-slate-300" :disabled="gapInstalling" @click="installGap"><LoaderCircle v-if="gapInstalling" class="size-4 animate-spin" />{{ gapInstalling ? '正在安装' : '继续安装并更新 Prompt' }}</button></div><p class="mt-2 text-xs text-amber-900">扫描仅辅助判断，是否继续安装由你决定。</p></div>
      </div>
      <div>
        <div class="flex items-center justify-between gap-3"><h2 class="text-sm font-semibold text-slate-800">实时预览</h2><button type="button" class="inline-flex h-9 items-center gap-2 border border-slate-300 bg-white px-3 text-sm font-medium text-slate-700 hover:border-teal-500 hover:text-teal-700 disabled:opacity-50" :disabled="!conversion" @click="copyPrompt"><Check v-if="copied" class="size-4 text-emerald-600" /><Clipboard v-else class="size-4" />{{ copied ? '已复制' : '复制' }}</button></div>
        <pre class="mt-2 min-h-80 whitespace-pre-wrap border border-slate-300 bg-white p-4 text-sm leading-6 text-slate-700">{{ loading ? '正在匹配已安装 skill...' : conversion?.prompt || '输入需求后将在此生成结构化 prompt。' }}</pre>
      </div>
    </div>
  </section>
</template>
