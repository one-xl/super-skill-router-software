param(
    [string]$Python = "tools\skillspector\.venv\Scripts\python.exe"
)

$ErrorActionPreference = "Stop"
$source = Join-Path $PSScriptRoot "..\tools\skillspector\SkillSpector-main\src"
$entryPoint = Join-Path $PSScriptRoot "skillspector_fast_sidecar.py"
$staging = "tmp\skillspector-fast-source"
$binaryName = "skillspector-fast-x86_64-pc-windows-msvc"

if (-not (Test-Path -LiteralPath $Python)) { throw "SkillSpector virtual environment was not found: $Python" }
if (-not (Test-Path -LiteralPath $source)) { throw "SkillSpector source was not found: $source" }

Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
Copy-Item -LiteralPath $source -Destination $staging -Recurse

# The upstream registry eagerly imports all semantic analyzers.  The fast build
# imports static modules explicitly, so its package initializer stays empty.
$registry = Join-Path $staging "skillspector\nodes\analyzers\__init__.py"
Set-Content -LiteralPath $registry -Encoding utf8 -Value '"""Static modules are imported directly by the fast sidecar."""'

# The full package initializer eagerly loads LangGraph and gets its version from
# wheel metadata, neither of which belongs in the static-only executable.
$packageInit = Join-Path $staging "skillspector\__init__.py"
Set-Content -LiteralPath $packageInit -Encoding utf8 -Value '__version__ = "2.9.1"'

# build_context only needs the file-size guard and a model-config placeholder;
# loading the provider registry here would pull the complete LLM stack back in.
$constants = Join-Path $staging "skillspector\constants.py"
Set-Content -LiteralPath $constants -Encoding utf8 -Value @'
MAX_FILE_BYTES = 1_000_000
def build_model_config() -> dict[str, str]:
    return {}
'@

# State annotations import this type even though fast mode never emits LLM
# telemetry. Keep the public type name without importing LangChain messages.
$inferenceUsage = Join-Path $staging "skillspector\inference_usage.py"
Set-Content -LiteralPath $inferenceUsage -Encoding utf8 -Value @'
from typing import TypedDict
class InferenceUsageRecord(TypedDict, total=False):
    node: str
'@

# Preserve upstream MCP TP1-TP3 code and replace the LLM-only TP4 section with
# a small static node. This avoids importing providers in the fast binary.
$mcpSource = Get-Content -LiteralPath (Join-Path $source "skillspector\nodes\analyzers\mcp_tool_poisoning.py") -Raw
$mcpPrefix = $mcpSource.Substring(0, $mcpSource.IndexOf('# TP4 placeholder'))
$mcpPrefix = $mcpPrefix -replace 'from pydantic import BaseModel, Field, field_validator\r?\n\r?\n', ''
$mcpPrefix = $mcpPrefix -replace 'from skillspector.inference_usage import InferenceUsageRecord\r?\n', ''
$mcpPrefix = $mcpPrefix -replace 'from skillspector.llm_analyzer_base import Batch, LLMAnalyzerBase\r?\n', ''
$mcpPrefix = $mcpPrefix -replace 'from skillspector.providers import get_active_provider\r?\n', ''
$mcpPrefix = $mcpPrefix -replace '    LLMCallRecord,\r?\n', ''
$mcpPrefix = $mcpPrefix -replace '    llm_call_record,\r?\n', ''
$mcpNode = @'
def node(state: SkillspectorState) -> AnalyzerNodeResponse:
    """Run upstream static MCP tool-poisoning checks TP1-TP3 only."""
    manifest: dict = state.get("manifest") or {}
    if not manifest:
        return {"findings": []}
    findings: list[Finding] = []
    for text, source_field, is_identifier in _extract_metadata_texts(manifest):
        findings.extend(_check_tp1(text, source_field))
        findings.extend(_check_tp2(text, source_field, is_identifier))
    findings.extend(_check_tp3(manifest.get("parameters") or []))
    return {"findings": findings}
'@
Set-Content -LiteralPath (Join-Path $staging "skillspector\nodes\analyzers\mcp_tool_poisoning_static.py") -Encoding utf8 -Value ($mcpPrefix + "`n" + $mcpNode)

& $Python -m PyInstaller --noconfirm --clean --onefile --name $binaryName `
    --paths $staging --collect-data skillspector `
    --hidden-import skillspector.nodes.analyzers.static_patterns_prompt_injection `
    --hidden-import skillspector.nodes.analyzers.static_patterns_data_exfiltration `
    --hidden-import skillspector.nodes.analyzers.static_patterns_privilege_escalation `
    --hidden-import skillspector.nodes.analyzers.static_patterns_supply_chain `
    --hidden-import skillspector.nodes.analyzers.static_patterns_harmful_content `
    --hidden-import skillspector.nodes.analyzers.static_patterns_excessive_agency `
    --hidden-import skillspector.nodes.analyzers.static_patterns_output_handling `
    --hidden-import skillspector.nodes.analyzers.static_patterns_system_prompt_leakage `
    --hidden-import skillspector.nodes.analyzers.static_patterns_memory_poisoning `
    --hidden-import skillspector.nodes.analyzers.static_patterns_tool_misuse `
    --hidden-import skillspector.nodes.analyzers.static_patterns_rogue_agent `
    --hidden-import skillspector.nodes.analyzers.static_patterns_agent_snooping `
    --hidden-import skillspector.nodes.analyzers.static_patterns_anti_refusal `
    --hidden-import skillspector.nodes.analyzers.static_patterns_ssrf `
    --hidden-import skillspector.nodes.analyzers.static_yara `
    --hidden-import skillspector.nodes.analyzers.behavioral_ast `
    --hidden-import skillspector.nodes.analyzers.behavioral_taint_tracking `
    --hidden-import skillspector.nodes.analyzers.mcp_least_privilege `
    --hidden-import skillspector.nodes.analyzers.mcp_tool_poisoning_static `
    --hidden-import skillspector.nodes.analyzers.mcp_rug_pull `
    --exclude-module langgraph --exclude-module langchain --exclude-module langchain_core `
    --exclude-module langchain_openai --exclude-module langchain_anthropic --exclude-module langchain_aws `
    --exclude-module openai --exclude-module anthropic --exclude-module boto3 --exclude-module botocore `
    --distpath "src-tauri\binaries" --workpath "tmp\pyinstaller-fast-work" `
    --specpath "tmp\pyinstaller-fast-spec" $entryPoint

if ($LASTEXITCODE -ne 0) { throw "PyInstaller fast sidecar build failed with exit code $LASTEXITCODE" }
