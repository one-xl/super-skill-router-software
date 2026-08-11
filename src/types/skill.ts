export interface SkillFile {
  path: string;
  size: number;
}

export interface SkillSource {
  repository: string;
  skillFilePath: string;
  ref: string;
  blobSha: string;
  rawUrl: string;
  discoverySource: string;
}

export interface Skill {
  id: string;
  name: string;
  description: string;
  whenToUse: string;
  tags: string[];
  repo: string;
  path: string;
  default_branch: string;
  commit_sha: string;
  files: SkillFile[];
  repo_size_kb: number;
  source: SkillSource;
}

export interface SkillIndex {
  schemaVersion: number;
  status: "complete" | "not_generated";
  generatedAt: string | null;
  query: string;
  sourceMatches: number;
  processedMatches: number;
  indexedSkills: number;
  truncated: boolean;
  skills: Skill[];
  warnings: string[];
}

export type SearchField = "name" | "whenToUse" | "description";

export interface SkillSearchResult {
  skill: Skill;
  score: number;
  matchedFields: SearchField[];
}
