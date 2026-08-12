<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { AlertCircle, CheckCircle2, LoaderCircle, MonitorCog, Play, Square } from "@lucide/vue";
import type { DesktopMonitorStatus } from "../types/skill";

const status = ref<DesktopMonitorStatus | null>(null);
const loading = ref(true);
const working = ref(false);
const error = ref<string | null>(null);
let unlisten: UnlistenFn | null = null;

const active = computed(() => status.value?.state === "watching" || status.value?.state === "reconnecting");

function fail(cause: unknown) {
  return typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "操作失败。";
}

function stateLabel(state: DesktopMonitorStatus["state"]) {
  return { watching: "正在监控", reconnecting: "正在重连", stopped: "已停止", error: "需要处理" }[state];
}

function stateClass(state: DesktopMonitorStatus["state"]) {
  return { watching: "text-emerald-700", reconnecting: "text-amber-700", stopped: "text-stone-400", error: "text-rose-700" }[state];
}

async function refresh() {
  const monitors = await invoke<DesktopMonitorStatus[]>("list_desktop_monitors");
  status.value = monitors.find((monitor) => monitor.target_id === "codex_desktop") ?? null;
}

async function start() {
  if (working.value) return;
  working.value = true;
  error.value = null;
  try {
    await invoke("start_desktop_monitor", { targetId: "codex_desktop" });
    await refresh();
  } catch (cause) { error.value = fail(cause); }
  finally { working.value = false; }
}

async function stop() {
  if (working.value || !active.value) return;
  working.value = true;
  error.value = null;
  try { await invoke("stop_desktop_monitor", { targetId: "codex_desktop" }); }
  catch (cause) { error.value = fail(cause); }
  finally { working.value = false; }
}

onMounted(async () => {
  try {
    unlisten = await listen<DesktopMonitorStatus>("desktop-monitor-status", ({ payload }) => {
      if (payload.target_id === "codex_desktop") status.value = payload;
    });
    await refresh();
  } catch (cause) { error.value = fail(cause); }
  finally { loading.value = false; }
});

onUnmounted(() => { unlisten?.(); });
</script>

<template>
  <section class="page-shell max-w-[980px]">
    <div class="page-header">
      <div>
        <p class="page-kicker">Monitor</p>
        <h1 class="page-title">桌面 Agent 自动恢复</h1>
        <p class="page-description">读取 ChatGPT Desktop 显示的重连状态；第 5 次失败后自动发送恢复指令。</p>
      </div>
      <button v-if="active" type="button" class="button-secondary" :disabled="working" @click="stop"><Square class="size-4" />停止监控</button>
      <button v-else type="button" class="button-primary" :disabled="working || loading" @click="start"><LoaderCircle v-if="working" class="size-4 animate-spin" /><Play v-else class="size-4" />开始监控</button>
    </div>

    <p v-if="error" class="notice-error mb-5" role="alert">{{ error }}</p>

    <section class="surface overflow-hidden">
      <div class="flex flex-wrap items-center justify-between gap-4 border-b border-stone-200 px-5 py-4">
        <div class="flex items-center gap-3">
          <span class="flex size-10 items-center justify-center rounded-md bg-stone-100 text-stone-700"><MonitorCog class="size-5" /></span>
          <div><h2 class="section-title">ChatGPT Desktop（Codex）</h2><p class="mt-1 text-[11px] text-stone-500">不启动 CLI，直接观察桌面应用自身日志。</p></div>
        </div>
        <span v-if="status" class="inline-flex items-center gap-1.5 text-[12px] font-medium" :class="stateClass(status.state)"><CheckCircle2 v-if="status.state === 'watching'" class="size-4" /><AlertCircle v-else class="size-4" />{{ stateLabel(status.state) }}</span>
        <span v-else class="text-[12px] text-stone-400">未运行</span>
      </div>

      <div class="grid sm:grid-cols-3">
        <div class="border-b border-stone-200 p-5 sm:border-b-0 sm:border-r"><p class="text-[10px] font-semibold uppercase tracking-wide text-stone-400">当前重连次数</p><p class="mt-2 text-2xl font-semibold" :class="status?.reconnect_attempt ? 'text-amber-700' : 'text-stone-900'">{{ status?.reconnect_attempt ?? 0 }} / 5</p></div>
        <div class="border-b border-stone-200 p-5 sm:border-b-0 sm:border-r"><p class="text-[10px] font-semibold uppercase tracking-wide text-stone-400">已发送恢复指令</p><p class="mt-2 text-2xl font-semibold text-teal-700">{{ status?.recovery_sent_count ?? 0 }}</p></div>
        <div class="p-5"><p class="text-[10px] font-semibold uppercase tracking-wide text-stone-400">自动发送内容</p><code class="mt-2 block text-[12px] text-stone-800">继续并恢复todo-list</code></div>
      </div>

      <div v-if="status?.last_error" class="notice-error m-5">{{ status.last_error }}</div>
      <div class="border-t border-stone-200 bg-stone-50 px-5 py-3 text-[11px] text-stone-500">
        <p>以 ChatGPT Desktop 对话框显示的“正在重新连接 x/5”为准；AppServer 日志仅用于兼容诊断。</p>
        <p v-if="status?.log_path" class="mt-1 truncate" :title="status.log_path">日志：{{ status.log_path }}</p>
      </div>
    </section>

    <section class="surface mt-5 p-5">
      <h2 class="section-title">Claude Code Desktop</h2>
      <p class="mt-2 text-[12px] leading-5 text-stone-500">支持从转换器填入 Prompt。当前版本未发现可稳定核验“第 5 次重连失败”的桌面日志字段，因此不启用自动发送，避免误触发。</p>
    </section>
  </section>
</template>
