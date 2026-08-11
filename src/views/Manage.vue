<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { AlertTriangle, Boxes, CloudUpload, Eye, LoaderCircle, MonitorCheck, RefreshCw, Trash2 } from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";
import MarkdownPreview from "../components/MarkdownPreview.vue";
import { deleteInstallationRecords, loadInstallationRecords } from "../lib/database";
import type { InstallationRecord, PreparedUninstall, TargetId, TargetSkillInventory } from "../types/skill";

type MatrixRow = { directory_name: string; records: InstallationRecord[]; live: TargetId[] };
const records = ref<InstallationRecord[]>([]);
const inventory = ref<TargetSkillInventory[]>([]);
const loading = ref(false);
const working = ref<string | null>(null);
const error = ref<string | null>(null);
const preview = ref<{ directoryName: string; name: string; content: string } | null>(null);
const previewLoading = ref<string | null>(null);
const pendingUninstall = ref<MatrixRow | null>(null);
const targets: TargetId[] = ["claude_code", "codex_cli", "codex_desktop", "claude_desktop"];
const labels: Record<TargetId, string> = { claude_code: "Claude Code", codex_cli: "Codex CLI", codex_desktop: "Codex Desktop", claude_desktop: "Claude Desktop" };

const rows = computed<MatrixRow[]>(() => {
  const names = new Set([...records.value.map((record) => record.directory_name), ...inventory.value.flatMap((item) => item.skills.map((skill) => skill.directory_name))]);
  return [...names].sort().map((directory_name) => ({ directory_name, records: records.value.filter((record) => record.directory_name === directory_name), live: targets.filter((id) => inventory.value.find((item) => item.id === id)?.skills.some((skill) => skill.directory_name === directory_name)) }));
});
const overview = computed(() => ({
  skills: rows.value.length,
  localTargets: new Set(rows.value.flatMap((row) => row.live)).size,
  pendingUploads: records.value.filter((record) => record.status === "packaged_for_upload").length,
  staleRecords: rows.value.reduce((total, row) => total + row.records.filter((record) => record.status === "installed" && !row.live.includes(record.target)).length, 0),
}));

function fail(cause: unknown) {
  return typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "操作失败，请重试。";
}

async function load() {
  loading.value = true;
  error.value = null;
  try { [records.value, inventory.value] = await Promise.all([loadInstallationRecords(), invoke<TargetSkillInventory[]>("list_installed_skills")]); }
  catch (cause) { error.value = fail(cause); }
  finally { loading.value = false; }
}

async function showPreview(row: MatrixRow) {
  if (preview.value?.directoryName === row.directory_name) {
    preview.value = null;
    return;
  }
  const target = row.live[0];
  if (!target || previewLoading.value) return;
  previewLoading.value = row.directory_name;
  error.value = null;
  try {
    preview.value = { directoryName: row.directory_name, name: row.records[0]?.skill_name ?? row.directory_name, content: await invoke<string>("read_installed_skill_markdown", { target, directoryName: row.directory_name }) };
  } catch (cause) { error.value = fail(cause); }
  finally { previewLoading.value = null; }
}

async function uninstall(row: MatrixRow) {
  if (!row.live.length || working.value) return;
  working.value = row.directory_name;
  error.value = null;
  let prepared: PreparedUninstall | null = null;
  try {
    prepared = await invoke<PreparedUninstall>("prepare_skill_uninstall", { directoryName: row.directory_name, targets: row.live });
    const dbTargets = row.records.filter((record) => record.status === "installed").map((record) => record.target);
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
  const row = pendingUninstall.value;
  if (!row) return;
  pendingUninstall.value = null;
  await uninstall(row);
}

onMounted(() => { void load(); });
</script>

<template>
  <section class="mx-auto w-full max-w-6xl px-6 py-8 lg:px-10">
    <div class="mb-5 flex items-end justify-between gap-4"><div><p class="mb-2 text-xs font-semibold uppercase tracking-[0.18em] text-teal-700">Manage</p><h1 class="text-3xl font-semibold text-slate-950">Skill 总览</h1><p class="mt-2 text-sm text-slate-500">矩阵以本机实际目录为准；Claude Desktop 的 zip 仅显示为待上传。</p></div><button class="flex size-9 items-center justify-center border border-slate-300 bg-white text-slate-600" title="刷新" :disabled="loading" @click="load"><RefreshCw class="size-4" :class="loading && 'animate-spin'" /></button></div>
    <div class="grid border border-slate-200 bg-white sm:grid-cols-4"><div class="flex items-center gap-3 border-b border-slate-200 p-4 sm:border-b-0 sm:border-r"><Boxes class="size-5 text-teal-600" /><div><p class="text-xl font-semibold text-slate-950">{{ overview.skills }}</p><p class="text-xs text-slate-500">已发现 Skill</p></div></div><div class="flex items-center gap-3 border-b border-slate-200 p-4 sm:border-b-0 sm:border-r"><MonitorCheck class="size-5 text-emerald-600" /><div><p class="text-xl font-semibold text-slate-950">{{ overview.localTargets }}</p><p class="text-xs text-slate-500">已同步目标端</p></div></div><div class="flex items-center gap-3 border-b border-slate-200 p-4 sm:border-b-0 sm:border-r"><CloudUpload class="size-5 text-amber-600" /><div><p class="text-xl font-semibold text-slate-950">{{ overview.pendingUploads }}</p><p class="text-xs text-slate-500">Claude 待上传</p></div></div><div class="flex items-center gap-3 p-4"><AlertTriangle class="size-5" :class="overview.staleRecords ? 'text-rose-600' : 'text-slate-400'" /><div><p class="text-xl font-semibold text-slate-950">{{ overview.staleRecords }}</p><p class="text-xs text-slate-500">需核对记录</p></div></div></div>
    <p v-if="error" class="mb-4 mt-5 border-l-4 border-rose-500 bg-rose-50 p-3 text-sm text-rose-900">{{ error }}</p>
    <section class="mt-5 overflow-x-auto border border-slate-200 bg-white"><div class="flex items-center justify-between border-b border-slate-200 px-4 py-3"><h2 class="text-sm font-semibold text-slate-900">多端同步矩阵</h2><span class="text-xs text-slate-500">预览会显示在选中 skill 的正下方</span></div><table class="min-w-full text-left text-sm"><thead class="bg-slate-50 text-xs text-slate-500"><tr><th class="px-4 py-3">Skill</th><th v-for="target in targets" :key="target" class="px-3 py-3">{{ labels[target] }}</th><th class="px-3 py-3">操作</th></tr></thead><tbody><template v-for="row in rows" :key="row.directory_name"><tr class="border-t border-slate-200"><td class="px-4 py-3 font-medium text-slate-900">{{ row.records[0]?.skill_name ?? row.directory_name }}<p v-if="row.records.some((record) => record.commit_sha !== '')" class="mt-1 text-xs font-normal text-slate-500">{{ row.directory_name }}</p></td><td v-for="target in targets" :key="target" class="px-3 py-3"><span v-if="row.live.includes(target)" class="text-emerald-700">已安装</span><span v-else-if="row.records.some((record) => record.target === target && record.status === 'packaged_for_upload')" class="text-amber-700">待上传</span><span v-else-if="row.records.some((record) => record.target === target)" class="text-rose-700">记录缺失</span><span v-else class="text-slate-300">-</span></td><td class="px-3 py-3"><div class="flex gap-2"><button v-if="row.live.length" class="flex size-8 items-center justify-center border border-slate-300 disabled:opacity-50" :title="preview?.directoryName === row.directory_name ? '关闭 SKILL.md 预览' : '预览 SKILL.md'" :disabled="previewLoading === row.directory_name" @click="showPreview(row)"><LoaderCircle v-if="previewLoading === row.directory_name" class="size-4 animate-spin" /><Eye v-else class="size-4" /></button><button v-if="row.live.length" class="flex size-8 items-center justify-center border border-rose-200 text-rose-700 disabled:opacity-50" title="卸载本地副本" :disabled="working === row.directory_name" @click="pendingUninstall = row"><LoaderCircle v-if="working === row.directory_name" class="size-4 animate-spin" /><Trash2 v-else class="size-4" /></button></div></td></tr><tr v-if="preview?.directoryName === row.directory_name" class="border-t border-teal-100 bg-teal-50/40"><td colspan="6" class="p-0"><div class="border-l-4 border-teal-500 bg-white"><div class="flex items-center justify-between border-b border-slate-200 px-4 py-3"><h3 class="font-semibold text-slate-900">{{ preview.name }} / SKILL.md</h3><button type="button" class="text-sm text-slate-500 hover:text-slate-900" @click="preview = null">关闭</button></div><MarkdownPreview :content="preview.content" /></div></td></tr></template><tr v-if="!loading && !rows.length"><td colspan="6" class="px-4 py-12 text-center text-slate-500">尚未检测到本地 skill。</td></tr></tbody></table></section>

    <div v-if="pendingUninstall" class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/35 p-5" role="dialog" aria-modal="true" aria-label="确认卸载 skill">
      <section class="w-full max-w-md border border-slate-200 bg-white p-6 shadow-xl">
        <div class="flex items-center gap-2 text-base font-semibold text-slate-950"><AlertTriangle class="size-5 text-rose-600" />确认卸载</div>
        <p class="mt-3 text-sm leading-6 text-slate-600">将卸载 <strong class="text-slate-900">{{ pendingUninstall.records[0]?.skill_name ?? pendingUninstall.directory_name }}</strong>，并从以下本地目标端删除完整 skill 目录：</p>
        <div class="mt-3 flex flex-wrap gap-2"><span v-for="target in pendingUninstall.live" :key="target" class="border border-slate-200 bg-slate-50 px-2 py-1 text-xs text-slate-700">{{ labels[target] }}</span></div>
        <p class="mt-3 text-xs text-rose-700">卸载后需要重新下载和扫描才能恢复。</p>
        <div class="mt-5 flex justify-end gap-2"><button type="button" class="h-9 border border-slate-300 bg-white px-3 text-sm text-slate-700" @click="pendingUninstall = null">取消</button><button type="button" class="inline-flex h-9 items-center gap-2 bg-rose-600 px-3 text-sm font-medium text-white hover:bg-rose-700" @click="confirmUninstall"><Trash2 class="size-4" />确认卸载</button></div>
      </section>
    </div>
  </section>
</template>
