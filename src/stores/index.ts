import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { searchSkills } from "../lib/search";
import type { Skill, SkillIndex, SkillSearchResult } from "../types/skill";
import { invoke } from "@tauri-apps/api/core";

const CACHE_KEY = "super-skill-router:index:v1";
const REMOVED_KEY = "super-skill-router:index-removed:v1";

function isSkillIndex(value: unknown): value is SkillIndex {
  return typeof value === "object" && value !== null && Array.isArray((value as SkillIndex).skills);
}

export const useSkillIndexStore = defineStore("skill-index", () => {
  const index = ref<SkillIndex | null>(null);
  const skills = ref<Skill[]>([]);
  const query = ref("");
  const activeTags = ref<string[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const usingOfflineCache = ref(false);
  const removedIds = ref<string[]>(readRemovedIds());
  const remoteSkills = ref<Skill[]>([]);
  const remoteLoading = ref(false);
  const remoteError = ref<string | null>(null);
  const remoteSearched = ref(false);

  const availableTags = computed(() => [...new Set(skills.value.flatMap((skill) => skill.tags))].sort((a, b) => a.localeCompare(b)));
  const results = computed(() => searchSkills(skills.value, query.value, activeTags.value));
  const removedCount = computed(() => index.value?.skills.filter((skill) => removedIds.value.includes(skill.id)).length ?? 0);
  const remoteResults = computed<SkillSearchResult[]>(() => remoteSkills.value.map((skill) => ({ skill, score: 0, matchedFields: [] })));

  function applyIndex(nextIndex: SkillIndex, offline: boolean) {
    index.value = nextIndex;
    skills.value = nextIndex.skills.filter((skill) => !removedIds.value.includes(skill.id));
    usingOfflineCache.value = offline;
  }

  function readRemovedIds() {
    try {
      const value: unknown = JSON.parse(localStorage.getItem(REMOVED_KEY) ?? "[]");
      return Array.isArray(value) ? value.filter((id): id is string => typeof id === "string") : [];
    } catch {
      localStorage.removeItem(REMOVED_KEY);
      return [];
    }
  }

  function removeSkills(ids: string[]) {
    const knownIds = new Set(index.value?.skills.map((skill) => skill.id) ?? []);
    removedIds.value = [...new Set([...removedIds.value, ...ids.filter((id) => knownIds.has(id))])];
    localStorage.setItem(REMOVED_KEY, JSON.stringify(removedIds.value));
    skills.value = skills.value.filter((skill) => !removedIds.value.includes(skill.id));
  }

  function restoreRemovedSkills() {
    removedIds.value = [];
    localStorage.removeItem(REMOVED_KEY);
    if (index.value) skills.value = [...index.value.skills];
  }

  async function load() {
    if (loading.value) {
      return;
    }
    loading.value = true;
    error.value = null;
    try {
      const response = await fetch("/skill-index.json", { cache: "no-store" });
      if (!response.ok) {
        throw new Error(`索引文件无法读取（HTTP ${response.status}）`);
      }
      const payload: unknown = await response.json();
      if (!isSkillIndex(payload) || payload.status !== "complete") {
        throw new Error("索引文件格式无效或尚未生成");
      }
      applyIndex(payload, false);
      localStorage.setItem(CACHE_KEY, JSON.stringify(payload));
    } catch (cause) {
      const cached = localStorage.getItem(CACHE_KEY);
      if (cached) {
        try {
          const payload: unknown = JSON.parse(cached);
          if (isSkillIndex(payload)) {
            applyIndex(payload, true);
            error.value = "无法读取最新索引，当前正在使用离线缓存。";
            return;
          }
        } catch {
          localStorage.removeItem(CACHE_KEY);
        }
      }
      skills.value = [];
      error.value = cause instanceof Error ? cause.message : "索引加载失败";
    } finally {
      loading.value = false;
    }
  }

  function toggleTag(tag: string) {
    activeTags.value = activeTags.value.includes(tag) ? activeTags.value.filter((value) => value !== tag) : [...activeTags.value, tag];
  }

  async function searchRemote() {
    if (remoteLoading.value) return;
    remoteError.value = null;
    if (query.value.trim().length < 2) {
      remoteError.value = "请输入至少两个字符后再搜索 SkillsMP。";
      return;
    }
    remoteLoading.value = true;
    remoteSearched.value = true;
    try {
      remoteSkills.value = await invoke<Skill[]>("search_skillsmp", { request: { query: query.value, limit: 20 } });
    } catch (cause) {
      remoteError.value = typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "远程搜索失败，请重试。";
    } finally {
      remoteLoading.value = false;
    }
  }

  return { index, skills, query, activeTags, loading, error, usingOfflineCache, availableTags, results, removedCount, remoteSkills, remoteLoading, remoteError, remoteSearched, remoteResults, load, toggleTag, removeSkills, restoreRemovedSkills, searchRemote };
});
