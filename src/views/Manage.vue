<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { AlertTriangle, Boxes, ChevronUp, CloudUpload, Eye, FolderOpen, LayoutGrid, LoaderCircle, MonitorCheck, RefreshCw, Trash2 } from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";
import SkillPreviewPanel from "../components/SkillPreviewPanel.vue";
import TargetIcon from "../components/TargetIcon.vue";
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
const views: Array<{ id: ManageView; label: string }> = [
  { id: "overview", label: "总览" },
  { id: "claude_code", label: "Claude Code" },
  { id: "codex_cli", label: "Codex CLI" },
  { id: "codex_desktop", label: "Codex Desktop" },
  { id: "claude_desktop", label: "Claude Desktop" },
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
  closePreview();
  error.value = null;
}

function targetForView(view: ManageView): TargetId {
  return view === "overview" ? "claude_code" : view;
}

function closePreview() {
  preview.value = null;
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
    closePreview();
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
  <section class="page-shell">
    <div class="page-header">
      <div><p class="page-kicker">Manage</p><h1 class="page-title">Skill 管理</h1><p class="page-description">查看全局同步状态，或按目标端核对本机 skill。</p></div>
      <button class="icon-button" title="刷新" :disabled="loading" @click="load"><RefreshCw class="size-4" :class="loading && 'animate-spin'" /></button>
    </div>

    <nav class="surface flex gap-1 overflow-x-auto p-1.5" aria-label="Skill 管理视图">
      <button v-for="item in views" :key="item.id" type="button" class="inline-flex h-11 shrink-0 items-center gap-2.5 rounded-md border px-3 text-[12px] transition duration-150" :class="activeView === item.id ? 'border-stone-200 bg-stone-50 font-semibold text-stone-950 shadow-sm' : 'border-transparent text-stone-500 hover:bg-stone-50 hover:text-stone-900'" :aria-current="activeView === item.id ? 'page' : undefined" @click="selectView(item.id)"><span v-if="item.id === 'overview'" class="flex size-8 items-center justify-center"><LayoutGrid class="size-[18px] text-teal-700" /></span><TargetIcon v-else :target="targetForView(item.id)" /><span>{{ item.label }}</span><span class="min-w-5 rounded-full bg-stone-100 px-1.5 py-0.5 text-center text-[10px]" :class="activeView === item.id ? 'text-stone-700' : 'text-stone-400'">{{ viewCount(item.id) }}</span></button>
    </nav>

    <p v-if="error" class="notice-error mb-4 mt-5">{{ error }}</p>

    <template v-if="activeView === 'overview'">
      <div class="surface mt-5 grid overflow-hidden sm:grid-cols-4">
        <div class="flex items-center gap-3 border-b border-stone-200 p-4 sm:border-b-0 sm:border-r"><span class="flex size-9 items-center justify-center rounded-md bg-teal-50 text-teal-700"><Boxes class="size-[18px]" /></span><div><p class="text-lg font-semibold text-stone-950">{{ overview.skills }}</p><p class="text-[11px] text-stone-500">已发现 Skill</p></div></div>
        <div class="flex items-center gap-3 border-b border-stone-200 p-4 sm:border-b-0 sm:border-r"><span class="flex size-9 items-center justify-center rounded-md bg-emerald-50 text-emerald-700"><MonitorCheck class="size-[18px]" /></span><div><p class="text-lg font-semibold text-stone-950">{{ overview.localTargets }}</p><p class="text-[11px] text-stone-500">已同步目标端</p></div></div>
        <div class="flex items-center gap-3 border-b border-stone-200 p-4 sm:border-b-0 sm:border-r"><span class="flex size-9 items-center justify-center rounded-md bg-amber-50 text-amber-700"><CloudUpload class="size-[18px]" /></span><div><p class="text-lg font-semibold text-stone-950">{{ overview.pendingUploads }}</p><p class="text-[11px] text-stone-500">Claude 待上传</p></div></div>
        <div class="flex items-center gap-3 p-4"><span class="flex size-9 items-center justify-center rounded-md" :class="overview.staleRecords ? 'bg-rose-50 text-rose-700' : 'bg-stone-100 text-stone-400'"><AlertTriangle class="size-[18px]" /></span><div><p class="text-lg font-semibold text-stone-950">{{ overview.staleRecords }}</p><p class="text-[11px] text-stone-500">需核对记录</p></div></div>
      </div>

      <section class="table-shell mt-5 overflow-x-auto">
        <div class="flex items-center justify-between border-b border-stone-200 px-4 py-3"><h2 class="section-title">多端同步矩阵</h2><span class="text-[11px] text-stone-500">{{ rows.length }} 个 Skill</span></div>
        <table class="data-table min-w-[56rem]">
          <colgroup><col class="w-[22%]" /><col v-for="target in targets" :key="target" class="w-[15%]" /><col class="w-[18%]" /></colgroup>
          <thead><tr><th class="px-4 py-3">Skill</th><th v-for="target in targets" :key="target" class="px-3 py-3"><span class="inline-flex items-center gap-2"><TargetIcon :target="target" compact />{{ labels[target] }}</span></th><th class="px-3 py-3">操作</th></tr></thead>
          <tbody>
            <template v-for="row in rows" :key="row.directory_name">
              <tr>
                <td class="px-4 py-3 font-medium text-stone-900">{{ rowName(row) }}<p v-if="row.records.some((record) => record.commit_sha !== '')" class="mt-1 text-[11px] font-normal text-stone-500">{{ row.directory_name }}</p></td>
                <td v-for="target in targets" :key="target" class="px-3 py-3"><span v-if="row.live.includes(target)" class="text-emerald-700">已安装</span><span v-else-if="row.records.some((record) => record.target === target && record.status === 'packaged_for_upload')" class="text-amber-700">待上传</span><span v-else-if="row.records.some((record) => record.target === target)" class="text-rose-700">记录缺失</span><span v-else class="text-slate-300">-</span></td>
                <td class="px-3 py-3"><div class="flex gap-2"><button v-if="row.live.length" type="button" class="icon-button size-8" :class="preview?.directoryName === row.directory_name ? 'border-teal-300 bg-teal-50 text-teal-800' : ''" :title="preview?.directoryName === row.directory_name ? '收起 SKILL.md 预览' : '预览 SKILL.md'" :aria-expanded="preview?.directoryName === row.directory_name" :disabled="previewLoading === row.directory_name" @click.stop="showOverviewPreview(row)"><LoaderCircle v-if="previewLoading === row.directory_name" class="size-4 animate-spin" /><ChevronUp v-else-if="preview?.directoryName === row.directory_name" class="size-4" /><Eye v-else class="size-4" /></button><button v-if="row.live.length" class="icon-button size-8 border-rose-200 text-rose-700 hover:border-rose-300 hover:bg-rose-50 hover:text-rose-800" title="卸载本地副本" :disabled="working === row.directory_name" @click="requestUninstall(row, 'overview')"><LoaderCircle v-if="working === row.directory_name" class="size-4 animate-spin" /><Trash2 v-else class="size-4" /></button></div></td>
              </tr>
              <tr v-if="preview?.directoryName === row.directory_name" class="preview-row"><td colspan="6" class="p-0"><SkillPreviewPanel :directory-name="preview.directoryName" :name="preview.name" :content="preview.content" @close="closePreview" /></td></tr>
            </template>
            <tr v-if="!loading && !rows.length"><td colspan="6" class="px-4 py-12 text-center text-slate-500">尚未检测到本地 skill。</td></tr>
          </tbody>
        </table>
      </section>
    </template>

    <template v-else>
      <section class="table-shell mt-5">
        <div class="flex flex-wrap items-center justify-between gap-3 border-b border-stone-200 px-4 py-3">
          <div class="flex items-center gap-3"><TargetIcon v-if="activeTarget" :target="activeTarget" /><div><h2 class="section-title">{{ activeTarget ? labels[activeTarget] : '' }}</h2><p class="mt-1 text-[11px] text-stone-500">{{ activeTarget === 'claude_desktop' ? '账号侧 skill 无法本地读取，此处仅列出已生成的待上传包。' : activeTarget === 'codex_cli' || activeTarget === 'codex_desktop' ? '与另一 Codex 客户端共享 CODEX_HOME。' : '本机实际目录中的 skill。' }}</p></div></div>
          <span class="text-[11px]" :class="activeInventory?.error ? 'text-rose-700' : 'text-stone-500'">{{ activeInventory?.error ?? `${applicationRows.length} 个 Skill` }}</span>
        </div>
        <div class="overflow-x-auto">
          <table class="data-table min-w-[48rem]">
            <colgroup><col class="w-[24%]" /><col class="w-[14%]" /><col class="w-[12%]" /><col class="w-[36%]" /><col class="w-[14%]" /></colgroup>
            <thead><tr><th class="px-4 py-3">Skill</th><th class="px-3 py-3">状态</th><th class="px-3 py-3">版本</th><th class="px-3 py-3">路径 / 上传包</th><th class="px-3 py-3">操作</th></tr></thead>
            <tbody>
              <template v-for="row in applicationRows" :key="row.directory_name">
                <tr>
                  <td class="px-4 py-3 font-medium text-stone-900">{{ rowName(row) }}<p class="mt-1 text-[11px] font-normal text-stone-500">{{ row.directory_name }}</p></td>
                  <td class="px-3 py-3"><span :class="statusClass(targetStatus(row))">{{ targetStatus(row) }}</span></td>
                  <td class="px-3 py-3 font-mono text-[11px] text-stone-500">{{ activeRecord(row)?.commit_sha ? activeRecord(row)?.commit_sha.slice(0, 8) : '-' }}</td>
                  <td class="max-w-sm px-3 py-3 text-[11px] text-stone-500"><span class="block truncate" :title="activePath(row) ?? activeRecord(row)?.package_path ?? ''">{{ activePath(row) ?? activeRecord(row)?.package_path ?? '-' }}</span></td>
                  <td class="px-3 py-3"><div class="flex gap-2"><button v-if="activeTarget && row.live.includes(activeTarget)" type="button" class="icon-button size-8" :class="preview?.directoryName === row.directory_name ? 'border-teal-300 bg-teal-50 text-teal-800' : ''" :title="preview?.directoryName === row.directory_name ? '收起 SKILL.md 预览' : '预览 SKILL.md'" :aria-expanded="preview?.directoryName === row.directory_name" :disabled="previewLoading === row.directory_name" @click.stop="showApplicationPreview(row)"><LoaderCircle v-if="previewLoading === row.directory_name" class="size-4 animate-spin" /><ChevronUp v-else-if="preview?.directoryName === row.directory_name" class="size-4" /><Eye v-else class="size-4" /></button><button v-if="activeTarget === 'claude_desktop' && activeRecord(row)?.package_path" class="icon-button size-8 border-amber-300 text-amber-800 hover:bg-amber-50" title="打开上传包所在目录" :disabled="packageOpening === row.directory_name" @click="activeRecord(row) && revealPackage(activeRecord(row)!)"><LoaderCircle v-if="packageOpening === row.directory_name" class="size-4 animate-spin" /><FolderOpen v-else class="size-4" /></button><button v-if="activeTarget && row.live.includes(activeTarget)" class="icon-button size-8 border-rose-200 text-rose-700 hover:border-rose-300 hover:bg-rose-50 hover:text-rose-800" title="从当前应用卸载" :disabled="working === row.directory_name" @click="requestUninstall(row, 'application')"><LoaderCircle v-if="working === row.directory_name" class="size-4 animate-spin" /><Trash2 v-else class="size-4" /></button></div></td>
                </tr>
                <tr v-if="preview?.directoryName === row.directory_name" class="preview-row"><td colspan="5" class="p-0"><SkillPreviewPanel :directory-name="preview.directoryName" :name="preview.name" :content="preview.content" @close="closePreview" /></td></tr>
              </template>
              <tr v-if="!loading && !applicationRows.length"><td colspan="5" class="px-4 py-12 text-center text-slate-500">{{ activeTarget === 'claude_desktop' ? '尚未生成待上传包。' : '当前应用尚未检测到 skill。' }}</td></tr>
            </tbody>
          </table>
        </div>
      </section>
    </template>

    <div v-if="pendingUninstall" class="modal-backdrop" role="dialog" aria-modal="true" aria-label="确认卸载 skill">
      <section class="modal-panel max-w-md">
        <div class="modal-icon-title"><span class="modal-icon bg-rose-50 text-rose-700"><AlertTriangle class="size-5" /></span>确认卸载</div>
        <p class="mt-4 text-sm leading-6 text-stone-600">将卸载 <strong class="text-stone-900">{{ rowName(pendingUninstall.row) }}</strong>，并从以下本地目标端删除完整 skill 目录：</p>
        <div class="mt-3 flex flex-wrap gap-2"><span v-for="target in pendingUninstall.targets" :key="target" class="inline-flex items-center gap-2 rounded-md border border-stone-200 bg-stone-50 px-2.5 py-1.5 text-[11px] text-stone-700"><TargetIcon :target="target" compact />{{ labels[target] }}</span></div>
        <p class="mt-3 text-xs text-rose-700">卸载后需要重新下载和扫描才能恢复。</p>
        <div class="mt-6 flex justify-end gap-2"><button type="button" class="button-ghost" @click="pendingUninstall = null">取消</button><button type="button" class="button-danger border-rose-600 bg-rose-600 text-white hover:bg-rose-700 hover:text-white" @click="confirmUninstall"><Trash2 class="size-4" />确认卸载</button></div>
      </section>
    </div>
  </section>
</template>
