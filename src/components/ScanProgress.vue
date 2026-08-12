<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { LoaderCircle } from "@lucide/vue";
import type { ScanMode } from "../types/skill";

const props = defineProps<{ active: boolean; mode: ScanMode }>();
const elapsed = ref(0);
let timer: number | undefined;

const progress = computed(() => {
  const duration = props.mode === "deep" ? 80 : 10;
  return Math.min(92, Math.round(8 + 84 * (1 - Math.exp(-elapsed.value / duration))));
});
const summary = computed(() => {
  if (props.mode === "deep") {
    if (elapsed.value < 5) return "正在校验完整 skill 目录与依赖文件";
    if (elapsed.value < 15) return "正在执行静态规则与依赖分析";
    if (elapsed.value < 60) return "正在进行 LLM 语义分析，可能需要 1–3 分钟";
    return "正在等待模型返回，请保持网络连接";
  }
  if (elapsed.value < 4) return "正在校验完整 skill 目录";
  return "正在执行裁剪版静态规则与脚本检查";
});

function start() {
  window.clearInterval(timer);
  elapsed.value = 0;
  timer = window.setInterval(() => { elapsed.value += 1; }, 1000);
}
watch(() => props.active, (active) => { if (active) start(); else window.clearInterval(timer); }, { immediate: true });
onBeforeUnmount(() => window.clearInterval(timer));
</script>

<template>
  <div v-if="active" class="mt-3 rounded-md border border-teal-100 bg-teal-50/70 px-3 py-2.5" aria-live="polite">
    <div class="flex items-center justify-between gap-3 text-[11px] font-medium text-teal-800"><span class="inline-flex items-center gap-1.5"><LoaderCircle class="size-3.5 animate-spin" />{{ mode === 'deep' ? '深度扫描进行中' : '快速扫描进行中' }}</span><span>{{ progress }}% · 近似进度</span></div>
    <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-teal-100"><div class="h-full rounded-full bg-teal-600 transition-[width] duration-700" :style="{ width: `${progress}%` }" /></div>
    <p class="mt-1.5 text-[10px] text-teal-700">{{ summary }}</p>
  </div>
</template>
