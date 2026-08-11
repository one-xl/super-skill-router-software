<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { AlertTriangle, Boxes, CloudUpload, Eye, FolderOpen, LayoutGrid, LoaderCircle, Monitor, MonitorCheck, RefreshCw, TerminalSquare, Trash2 } from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";
import SkillPreviewPanel from "../components/SkillPreviewPanel.vue";
import { deleteInstallationRecords, loadInstallationRecords } from "../lib/database";
import type { InstallationRecord, PreparedUninstall, TargetId, TargetSkillInventory } from "../types/skill";

type MatrixRow = { directory_name: string; records: InstallationRecord[]; live: TargetId[] };
type ManageView = "overview" | TargetId;
type PendingUninstall = { row: MatrixRow; targets: TargetId[] };

const records = ref<InstallationRecord[]>([]);
const inventory = ref<TargetSkillInventory[]>([]);
const loading = ref(false);
const working = ref<string | null>(null);
const packageOpening = ref<string | null>(null);
const error = ref<string | null>(null);
const preview = ref<{ directoryName: string; name: string; content: string } | null>(null);
const previewLoading = ref<string | null>(null);
const pendingUninstall = ref<PendingUninstall | null>(null);
const activeView = ref<ManageView>("overview");

const targets: TargetId[] = ["claude_code", "codex_cli", "codex_desktop", "claude_desktop"];
const labels: Record<TargetId, string> = { claude_code: "Claude Code", codex_cli: "Codex CLI", codex_desktop: "Codex Desktop", claude_desktop: "Claude Desktop" };
const views: Array<{ id: ManageView; label: string; icon: typeof LayoutGrid }> = [
  { id: "overview", label: "总览", icon: LayoutGrid },
  { id: "claude_code", label: "Claude Code", icon: TerminalSquare },
  { id: "codex_cli", label: "Codex CLI", icon: TerminalSquare },
  { id: "codex_desktop", label: "Codex Desktop", icon: Monitor },
  { id: "claude_desktop", label: "Claude Desktop", icon: Monitor },
];

const rows = computed<MatrixRow[]>(() => {
  const names = new Set([...records.value.map((record) => record.directory_name), ...inventory.value.flatMap((item) => item.skills.map((skill) => skill.directory_name))]);
  return [...names].sort().map((directory_name) => ({
    directory_name,
    records: records.value.filter((record) => record.directory_name === directory_name),
    live: targets.filter((id) => inventory.value.find((item) => item.id === id)?.skills.some((skill) => skill.directory_name === directory_name)),
  }));
});
const overview = computed(() => ({
  skills: rows.value.length,
  localTargets: new Set(rows.value.flatMap((row) => row.live)).size,
  pendingUploads: records.value.filter((record) => record.status === "packaged_for_upload").length,
  staleRecords: rows.value.reduce((total, row) => total + row.records.filter((record) => record.status === "installed" && !row.live.includes(record.target)).length, 0),
}));
const activeTarget = computed<TargetId | null>(() => activeView.value === "overview" ? null : activeView.value);
const activeInventory = computed(() => activeTarget.value ? inventory.value.find((item) => item.id === activeTarget.value) ?? null : null);
const applicationRows = computed(() => {
  const target = activeTarget.value;
  if (!target) return [];
  return rows.value.filter((row) => row.live.includes(target) || row.records.some((record) => record.target === target));
});

function fail(cause: unknown) {
  return typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "操作失败，请重试。";
}

function selectView(view: ManageView) {
  activeView.value = view;
  preview.value = null;
  error.value = null;
}

function viewCount(view: ManageView) {
  if (view === "overview") return rows.value.length;
  if (view === "claude_desktop") return rows.value.filter((row) => row.records.some((record) => record.target === view)).length;
  return inventory.value.find((item) => item.id === view)?.skills.length ?? 0;
}

function rowName(row: MatrixRow) {
  return row.records[0]?.skill_name ?? row.directory_name;
}

function activeRecord(row: MatrixRow) {
  return activeTarget.value ? row.records.find((record) => record.target === activeTarget.value) ?? null : null;
}

function activePath(row: MatrixRow) {
  const target = activeTarget.value;
  if (!target) return null;
  return inventory.value.find((item) => item.id === target)?.skills.find((skill) => skill.directory_name === row.directory_name)?.path ?? null;
}

function targetStatus(row: MatrixRow) {
  const target = activeTarget.value;
  const record = activeRecord(row);
  if (!target) return "";
  if (target === "claude_desktop" && record?.status === "packaged_for_upload") return "待上传";
  if (row.live.includes(target)) return record ? "已安装" : "已检测";
  if (record) return "记录缺失";
  return "未安装";
}

function statusClass(status: string) {
  if (status === "已安装" || status === "已检测") return "text-emerald-700";
  if (status === "待上传") return "text-amber-700";
  if (status === "记录缺失") return "text-rose-700";
  return "text-slate-500";
}

async function load() {
  loading.value = true;
  error.value = null;
  try { [records.value, inventory.value] = await Promise.all([loadInstallationRecords(), invoke<TargetSkillInventory[]>("list_installed_skills")]); }
  catch (cause) { error.value = fail(cause); }
  finally { loading.value = false; }
}

async function showPreview(row: MatrixRow, target: TargetId) {
  if (preview.value?.directoryName === row.directory_name) {
    preview.value = null;
    return;
  }
  if (!row.live.includes(target) || previewLoading.value) return;
  previewLoading.value = row.directory_name;
  error.value = null;
  try {
    const content = await invoke<string>("read_installed_skill_markdown", { target, directoryName: row.directory_name });
    preview.value = { directoryName: row.directory_name, name: rowName(row), content };
  } catch (cause) { error.value = fail(cause); }
  finally { previewLoading.value = null; }
}

function showOverviewPreview(row: MatrixRow) {
  const target = row.live[0];
  if (target) void showPreview(row, target);
}

function showApplicationPreview(row: MatrixRow) {
  const target = activeTarget.value;
  if (target) void showPreview(row, target);
}

function requestUninstall(row: MatrixRow, scope: "overview" | "application") {
  let affected = [...row.live];
  const target = activeTarget.value;
  if (scope === "application" && target) {
    affected = target === "codex_cli" || target === "codex_desktop"
      ? row.live.filter((id) => id === "codex_cli" || id === "codex_desktop")
      : row.live.filter((id) => id === target);
  }
  if (affected.length) pendingUninstall.value = { row, targets: affected };
}

async function uninstall(row: MatrixRow, affectedTargets: TargetId[]) {
  if (!affectedTargets.length || working.value) return;
  working.value = row.directory_name;
  error.value = null;
  let prepared: PreparedUninstall | null = null;
  try {
    prepared = await invoke<PreparedUninstall>("prepare_skill_uninstall", { directoryName: row.directory_name, targets: affectedTargets });
    const dbTargets = row.records.filter((record) => record.status === "installed" && affectedTargets.includes(record.target)).map((record) => record.target);
    await deleteInstallationRecords(row.directory_name, dbTargets);
    await invoke("commit_skill_uninstall", { token: prepared.token });
    if (preview.value?.directoryName === row.directory_name) preview.value = null;
    await load();
  } catch (cause) {
    if (prepared) try { await invoke("rollback_skill_uninstall", { token: prepared.token }); } catch { /* Preserve the original error. */ }
    error.value = fail(cause);
  } finally { working.value = null; }
}

async function confirmUninstall() {
  const pending = pendingUninstall.value;
  if (!pending) return;
  pendingUninstall.value = null;
  await uninstall(pending.row, pending.targets);
}

async function revealPackage(record: InstallationRecord) {
  if (!record.package_path || packageOpening.value) return;
  packageOpening.value = record.directory_name;
  error.value = null;
  try { await invoke("reveal_packaged_skill", { zipPath: record.package_path }); }
  catch (cause) { error.value = fail(cause); }
  finally { packageOpening.value = null; }
}

onMounted(() => { void load(); });
</script>

<template>
  <section class="mx-auto w-full max-w-6xl px-6 py-8 lg:px-10">
    <div class="mb-5 flex items-end justify-between gap-4">
      <div><p class="mb-2 text-xs font-semibold uppercase tracking-[0.18em] text-teal-700">Manage</p><h1 class="text-3xl font-semibold text-slate-950">Skill 管理</h1><p class="mt-2 text-sm text-slate-500">查看总览或按应用核对本机 skill。</p></div>
      <button class="flex size-9 items-center justify-center border border-slate-300 bg-white text-slate-600" title="刷新" :disabled="loading" @click="load"><RefreshCw class="size-4" :class="loading && 'animate-spin'" /></button>
    </div>

    <nav class="flex overflow-x-auto border-b border-slate-200" aria-label="Skill 管理视图">
      <button v-for="view in views" :key="view.id" type="button" class="inline-flex h-11 shrink-0 items-center gap-2 border-b-2 px-4 text-sm transition" :class="activeView === view.id ? 'border-teal-600 font-semibold text-teal-800' : 'border-transparent text-slate-500 hover:text-slate-900'" :aria-current="activeView === view.id ? 'page' : undefined" @click="selectView(view.id)"><component :is="view.icon" class="size-4" /><span>{{ view.label }}</span><span class="min-w-5 text-center text-xs" :class="activeView === view.id ? 'text-teal-700' : 'text-slate-400'">{{ viewCount(view.id) }}</span></button>
    </nav>

    <p v-if="error" class="mb-4 mt-5 border-l-4 border-rose-500 bg-rose-50 p-3 text-sm text-rose-900">{{ error }}</p>

    <template v-if="activeView === 'overview'">
      <div class="mt-5 grid border border-slate-200 bg-white sm:grid-cols-4">
        <div class="flex items-center gap-3 border-b border-slate-200 p-4 sm:border-b-0 sm:border-r"><Boxes class="size-5 text-teal-600" /><div><p class="text-xl font-semibold text-slate-950">{{ overview.skills }}</p><p class="text-xs text-slate-500">已发现 Skill</p></div></div>
        <div class="flex items-center gap-3 border-b border-slate-200 p-4 sm:border-b-0 sm:border-r"><MonitorCheck class="size-5 text-emerald-600" /><div><p class="text-xl font-semibold text-slate-950">{{ overview.localTargets }}</p><p class="text-xs text-slate-500">已同步目标端</p></div></div>
        <div class="flex items-center gap-3 border-b border-slate-200 p-4 sm:border-b-0 sm:border-r"><CloudUpload class="size-5 text-amber-600" /><div><p class="text-xl font-semibold text-slate-950">{{ overview.pendingUploads }}</p><p class="text-xs text-slate-500">Claude 待上传</p></div></div>
        <div class="flex items-center gap-3 p-4"><AlertTriangle class="size-5" :class="overview.staleRecords ? 'text-rose-600' : 'text-slate-400'" /><div><p class="text-xl font-semibold text-slate-950">{{ overview.staleRecords }}</p><p class="text-xs text-slate-500">需核对记录</p></div></div>
      </div>

      <section class="mt-5 overflow-x-auto border border-slate-200 bg-white">
        <div class="flex items-center justify-between border-b border-slate-200 px-4 py-3"><h2 class="text-sm font-semibold text-slate-900">多端同步矩阵</h2><span class="text-xs text-slate-500">预览会显示在选中 skill 的正下方</span></div>
        <table class="min-w-full text-left text-sm">
          <thead class="bg-slate-50 text-xs text-slate-500"><tr><th class="px-4 py-3">Skill</th><th v-for="target in targets" :key="target" class="px-3 py-3">{{ labels[target] }}</th><th class="px-3 py-3">操作</th></tr></thead>
          <tbody>
            <template v-for="row in rows" :key="row.directory_name">
              <tr class="border-t border-slate-200">
                <td class="px-4 py-3 font-medium text-slate-900">{{ rowName(row) }}<p v-if="row.records.some((record) => record.commit_sha !== '')" class="mt-1 text-xs font-normal text-slate-500">{{ row.directory_name }}</p></td>
                <td v-for="target in targets" :key="target" class="px-3 py-3"><span v-if="row.live.includes(target)" class="text-emerald-700">已安装</span><span v-else-if="row.records.some((record) => record.target === target && record.status === 'packaged_for_upload')" class="text-amber-700">待上传</span><span v-else-if="row.records.some((record) => record.target === target)" class="text-rose-700">记录缺失</span><span v-else class="text-slate-300">-</span></td>
                <td class="px-3 py-3"><div class="flex gap-2"><button v-if="row.live.length" class="flex size-8 items-center justify-center border border-slate-300 disabled:opacity-50" :title="preview?.directoryName === row.directory_name ? '关闭 SKILL.md 预览' : '预览 SKILL.md'" :disabled="previewLoading === row.directory_name" @click="showOverviewPreview(row)"><LoaderCircle v-if="previewLoading === row.directory_name" class="size-4 animate-spin" /><Eye v-else class="size-4" /></button><button v-if="row.live.length" class="flex size-8 items-center justify-center border border-rose-200 text-rose-700 disabled:opacity-50" title="卸载本地副本" :disabled="working === row.directory_name" @click="requestUninstall(row, 'overview')"><LoaderCircle v-if="working === row.directory_name" class="size-4 animate-spin" /><Trash2 v-else class="size-4" /></button></div></td>
              </tr>
              <tr v-if="preview?.directoryName === row.directory_name" class="border-t border-teal-100 bg-teal-50/40"><td colspan="6" class="p-0"><SkillPreviewPanel :directory-name="preview.directoryName" :name="preview.name" :content="preview.content" @close="preview = null" /></td></tr>
            </template>
            <tr v-if="!loading && !rows.length"><td colspan="6" class="px-4 py-12 text-center text-slate-500">尚未检测到本地 skill。</td></tr>
          </tbody>
        </table>
      </section>
    </template>

    <template v-else>
      <section class="mt-5 border border-slate-200 bg-white">
        <div class="flex flex-wrap items-center justify-between gap-3 border-b border-slate-200 px-4 py-3">
          <div><h2 class="text-sm font-semibold text-slate-900">{{ activeTarget ? labels[activeTarget] : '' }}</h2><p class="mt-1 text-xs text-slate-500">{{ activeTarget === 'claude_desktop' ? '账号侧 skill 无法本地读取，此处仅列出已生成的待上传包。' : activeTarget === 'codex_cli' || activeTarget === 'codex_desktop' ? '与另一 Codex 客户端共享 CODEX_HOME。' : '本机实际目录中的 skill。' }}</p></div>
          <span class="text-xs" :class="activeInventory?.error ? 'text-rose-700' : 'text-slate-500'">{{ activeInventory?.error ?? `${applicationRows.length} 个 Skill` }}</span>
        </div>
        <div class="overflow-x-auto">
          <table class="min-w-full text-left text-sm">
            <thead class="bg-slate-50 text-xs text-slate-500"><tr><th class="px-4 py-3">Skill</th><th class="px-3 py-3">状态</th><th class="px-3 py-3">版本</th><th class="px-3 py-3">路径 / 上传包</th><th class="px-3 py-3">操作</th></tr></thead>
            <tbody>
              <template v-for="row in applicationRows" :key="row.directory_name">
                <tr class="border-t border-slate-200">
                  <td class="px-4 py-3 font-medium text-slate-900">{{ rowName(row) }}<p class="mt-1 text-xs font-normal text-slate-500">{{ row.directory_name }}</p></td>
                  <td class="px-3 py-3"><span :class="statusClass(targetStatus(row))">{{ targetStatus(row) }}</span></td>
                  <td class="px-3 py-3 text-xs text-slate-500">{{ activeRecord(row)?.commit_sha ? activeRecord(row)?.commit_sha.slice(0, 8) : '-' }}</td>
                  <td class="max-w-sm px-3 py-3 text-xs text-slate-500"><span class="block truncate" :title="activePath(row) ?? activeRecord(row)?.package_path ?? ''">{{ activePath(row) ?? activeRecord(row)?.package_path ?? '-' }}</span></td>
                  <td class="px-3 py-3"><div class="flex gap-2"><button v-if="activeTarget && row.live.includes(activeTarget)" class="flex size-8 items-center justify-center border border-slate-300 disabled:opacity-50" :title="preview?.directoryName === row.directory_name ? '关闭 SKILL.md 预览' : '预览 SKILL.md'" :disabled="previewLoading === row.directory_name" @click="showApplicationPreview(row)"><LoaderCircle v-if="previewLoading === row.directory_name" class="size-4 animate-spin" /><Eye v-else class="size-4" /></button><button v-if="activeTarget === 'claude_desktop' && activeRecord(row)?.package_path" class="flex size-8 items-center justify-center border border-amber-300 text-amber-800 disabled:opacity-50" title="打开上传包所在目录" :disabled="packageOpening === row.directory_name" @click="activeRecord(row) && revealPackage(activeRecord(row)!)"><LoaderCircle v-if="packageOpening === row.directory_name" class="size-4 animate-spin" /><FolderOpen v-else class="size-4" /></button><button v-if="activeTarget && row.live.includes(activeTarget)" class="flex size-8 items-center justify-center border border-rose-200 text-rose-700 disabled:opacity-50" title="从当前应用卸载" :disabled="working === row.directory_name" @click="requestUninstall(row, 'application')"><LoaderCircle v-if="working === row.directory_name" class="size-4 animate-spin" /><Trash2 v-else class="size-4" /></button></div></td>
                </tr>
                <tr v-if="preview?.directoryName === row.directory_name" class="border-t border-teal-100 bg-teal-50/40"><td colspan="5" class="p-0"><SkillPreviewPanel :directory-name="preview.directoryName" :name="preview.name" :content="preview.content" @close="preview = null" /></td></tr>
              </template>
              <tr v-if="!loading && !applicationRows.length"><td colspan="5" class="px-4 py-12 text-center text-slate-500">{{ activeTarget === 'claude_desktop' ? '尚未生成待上传包。' : '当前应用尚未检测到 skill。' }}</td></tr>
            </tbody>
          </table>
        </div>
      </section>
    </template>

    <div v-if="pendingUninstall" class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/35 p-5" role="dialog" aria-modal="true" aria-label="确认卸载 skill">
      <section class="w-full max-w-md border border-slate-200 bg-white p-6 shadow-xl">
        <div class="flex items-center gap-2 text-base font-semibold text-slate-950"><AlertTriangle class="size-5 text-rose-600" />确认卸载</div>
        <p class="mt-3 text-sm leading-6 text-slate-600">将卸载 <strong class="text-slate-900">{{ rowName(pendingUninstall.row) }}</strong>，并从以下本地目标端删除完整 skill 目录：</p>
        <div class="mt-3 flex flex-wrap gap-2"><span v-for="target in pendingUninstall.targets" :key="target" class="border border-slate-200 bg-slate-50 px-2 py-1 text-xs text-slate-700">{{ labels[target] }}</span></div>
        <p class="mt-3 text-xs text-rose-700">卸载后需要重新下载和扫描才能恢复。</p>
        <div class="mt-5 flex justify-end gap-2"><button type="button" class="h-9 border border-slate-300 bg-white px-3 text-sm text-slate-700" @click="pendingUninstall = null">取消</button><button type="button" class="inline-flex h-9 items-center gap-2 bg-rose-600 px-3 text-sm font-medium text-white hover:bg-rose-700" @click="confirmUninstall"><Trash2 class="size-4" />确认卸载</button></div>
      </section>
    </div>
  </section>
</template>
