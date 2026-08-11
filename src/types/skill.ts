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

export type ScanMode = "fast" | "deep";

export interface ScanIssue {
  id: string;
  category?: string | null;
  severity: string;
  confidence: number;
  location: { file: string; start_line: number; end_line?: number | null };
  explanation: string;
  remediation?: string | null;
  code_snippet?: string | null;
}

export interface ScanReport {
  risk_assessment: { score: number; severity: string; recommendation: string };
  issues: ScanIssue[];
  execution_successful: boolean;
  analysis_completeness: unknown;
}

export interface PreparedInstall {
  token: string;
  directory_name: string;
}

export type ApiFormat = "openai" | "anthropic";
export interface ApiConfig { format: ApiFormat; apiUrl: string; apiKey: string; model: string; }
export interface AppSettings { deepScan: ApiConfig; prompt: ApiConfig; }

export interface InstallOutcome {
  kind: "installed" | "packaged_for_upload";
  path?: string;
  zip_path?: string;
}

export type TargetId = "claude_code" | "codex_cli" | "codex_desktop" | "claude_desktop";

export interface TargetDetection {
  id: TargetId;
  name: string;
  path: string | null;
  available: boolean;
}

export interface TargetInstallResult {
  target: TargetId;
  target_name: string;
  outcome: InstallOutcome | null;
  error: string | null;
  reused_physical_install: boolean;
}

export interface BatchInstallReport {
  results: TargetInstallResult[];
}

export interface ConverterSkill {
  id: string;
  name: string;
  description: string;
  whenToUse: string;
  tags: string[];
  frecency?: number;
}

export interface ScoredConverterSkill extends ConverterSkill {
  score: number;
}

export interface ConversionResult {
  scenario: "coding" | "refactor" | "debug" | "review" | "generic";
  prompt: string;
  selected: ScoredConverterSkill[];
  gaps: ScoredConverterSkill[];
}

export interface InstallationRecord {
  skill_name: string;
  directory_name: string;
  repository: string;
  commit_sha: string;
  target: TargetId;
  status: "installed" | "packaged_for_upload";
  source_url?: string;
  installed_path?: string | null;
  package_path?: string | null;
  installed_at?: string;
  updated_at?: string;
}

export interface TargetSkillInventory { id: TargetId; name: string; skills: Array<{ directory_name: string; path: string }>; error: string | null; }
export interface PreparedUninstall { token: string; staged_targets: TargetId[]; }
