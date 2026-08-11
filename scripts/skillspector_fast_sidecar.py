"""Trimmed static-only entry point for NVIDIA SkillSpector.

This runner intentionally scans a complete skill directory.  It reuses the
upstream static, YARA, behavioural and MCP (TP1-TP3) analyzers, but does not
load any LLM provider or LangGraph workflow code.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any

from skillspector.nodes.build_context import build_context
from skillspector.nodes.deduplicate import deduplicate


STATIC_ANALYZERS = (
    "static_patterns_prompt_injection",
    "static_patterns_data_exfiltration",
    "static_patterns_privilege_escalation",
    "static_patterns_supply_chain",
    "static_patterns_harmful_content",
    "static_patterns_excessive_agency",
    "static_patterns_output_handling",
    "static_patterns_system_prompt_leakage",
    "static_patterns_memory_poisoning",
    "static_patterns_tool_misuse",
    "static_patterns_rogue_agent",
    "static_patterns_agent_snooping",
    "static_patterns_anti_refusal",
    "static_patterns_ssrf",
    "static_yara",
    "behavioral_ast",
    "behavioral_taint_tracking",
    "mcp_least_privilege",
    "mcp_tool_poisoning_static",
    "mcp_rug_pull",
)


def _risk_assessment(findings: list[Any], has_executable_scripts: bool) -> tuple[int, str, str]:
    """Use the upstream documented severity weights and diminishing returns."""
    base_points = {"CRITICAL": 50.0, "HIGH": 25.0, "MEDIUM": 10.0, "LOW": 5.0}
    severity_rank = {"CRITICAL": 0, "HIGH": 1, "MEDIUM": 2, "LOW": 3}
    executable_extensions = {".py", ".sh", ".bash", ".zsh", ".js", ".ts", ".rb", ".go", ".rs", ".pl"}
    by_rule: dict[str, list[Any]] = defaultdict(list)
    for finding in findings:
        by_rule[str(finding.rule_id)].append(finding)

    total = 0.0
    for rule_findings in by_rule.values():
        rule_findings.sort(key=lambda item: severity_rank.get(str(item.severity).upper(), 4))
        for occurrence, finding in enumerate(rule_findings[:3]):
            confidence = max(0.0, min(1.0, float(finding.confidence)))
            multiplier = (1.0, 0.5, 0.25)[occurrence]
            suffix = Path(str(finding.file)).suffix.lower()
            executable_multiplier = 1.3 if suffix in executable_extensions and has_executable_scripts else 1.0
            total += base_points.get(str(finding.severity).upper(), 0.0) * confidence * multiplier * executable_multiplier

    score = min(100, round(total))
    if score >= 81:
        return score, "CRITICAL", "DO_NOT_INSTALL"
    if score >= 51:
        return score, "HIGH", "DO_NOT_INSTALL"
    if score >= 21:
        return score, "MEDIUM", "CAUTION"
    return score, "LOW", "SAFE"


def scan(skill_path: Path) -> dict[str, object]:
    if not skill_path.is_dir():
        raise ValueError(f"Skill directory does not exist: {skill_path}")

    state: dict[str, Any] = {"skill_path": str(skill_path), "use_llm": False}
    state.update(build_context(state))
    findings: list[Any] = []
    errors: list[str] = []

    for analyzer_name in STATIC_ANALYZERS:
        module = __import__(f"skillspector.nodes.analyzers.{analyzer_name}", fromlist=["node"])
        try:
            response = module.node(state)
            findings.extend(response.get("findings", []))
        except Exception as error:  # The report must distinguish incomplete static analysis.
            errors.append(f"{analyzer_name}: {type(error).__name__}: {error}")

    filtered = deduplicate(findings)
    score, severity, recommendation = _risk_assessment(filtered, bool(state.get("has_executable_scripts")))
    return {
        "scanner": {"name": "SkillSpector", "mode": "fast", "llm_used": False},
        "skill": {"name": state.get("manifest", {}).get("name", skill_path.name), "path": str(skill_path)},
        "risk_assessment": {"score": score, "severity": severity, "recommendation": recommendation},
        "issues": [finding.to_dict() for finding in filtered],
        "execution_successful": not errors,
        "analysis_completeness": {
            "scanned_components": len(state.get("components", [])),
            "analyzer_errors": errors,
            "llm_analysis": "not_run",
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="SkillSpector fast static scanner")
    subparsers = parser.add_subparsers(dest="command", required=True)
    command = subparsers.add_parser("scan")
    command.add_argument("skill_path")
    command.add_argument("--format", choices=("json",), default="json")
    command.add_argument("--output", type=Path)
    args = parser.parse_args()

    try:
        report = scan(Path(args.skill_path))
    except (OSError, ValueError) as error:
        print(json.dumps({"execution_successful": False, "error": str(error)}), file=sys.stderr)
        return 2

    payload = json.dumps(report, ensure_ascii=False, indent=2)
    if args.output:
        args.output.write_text(payload, encoding="utf-8")
    else:
        print(payload)
    return 1 if report["risk_assessment"]["score"] > 50 else 0


if __name__ == "__main__":
    raise SystemExit(main())
