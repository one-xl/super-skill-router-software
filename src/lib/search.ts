import uFuzzy from "@leeoniya/ufuzzy";
import type { SearchField, Skill, SkillSearchResult } from "../types/skill";

const fields: ReadonlyArray<{ key: SearchField; weight: number }> = [
  { key: "name", weight: 3.0 },
  { key: "whenToUse", weight: 2.5 },
  { key: "description", weight: 2.0 },
];

const fuzzy = new uFuzzy({
  unicode: true,
  intraMode: 1,
  interLft: 1,
});

export function searchSkills(skills: Skill[], query: string, activeTags: string[]): SkillSearchResult[] {
  const needle = query.trim();
  const tagSet = new Set(activeTags);
  const candidates = tagSet.size === 0 ? skills : skills.filter((skill) => skill.tags.some((tag) => tagSet.has(tag)));

  if (!needle) {
    return candidates.map((skill) => ({ skill, score: 0, matchedFields: [] }));
  }

  const results = new Map<string, SkillSearchResult>();
  const loweredNeedle = needle.toLocaleLowerCase();

  for (const { key, weight } of fields) {
    for (const skill of candidates) {
      if (!skill[key].toLocaleLowerCase().includes(loweredNeedle)) {
        continue;
      }
      const existing = results.get(skill.id) ?? { skill, score: 0, matchedFields: [] };
      existing.score += weight * 5;
      if (!existing.matchedFields.includes(key)) {
        existing.matchedFields.push(key);
      }
      results.set(skill.id, existing);
    }

    const haystack = candidates.map((skill) => skill[key]);
    const [matches, info, order] = fuzzy.search(haystack, needle, 2);
    if (matches === null || matches.length === 0) {
      continue;
    }
    const ranked = info !== null && order !== null ? order.map((position) => info.idx[position]) : matches;
    ranked.forEach((candidateIndex, rank) => {
      const skill = candidates[candidateIndex];
      if (!skill) {
        return;
      }
      const existing = results.get(skill.id) ?? { skill, score: 0, matchedFields: [] };
      const rankBoost = weight * (1 - rank / Math.max(ranked.length, 1));
      existing.score += weight + rankBoost;
      if (!existing.matchedFields.includes(key)) {
        existing.matchedFields.push(key);
      }
      results.set(skill.id, existing);
    });
  }

  return [...results.values()].sort((left, right) => right.score - left.score || left.skill.name.localeCompare(right.skill.name));
}
