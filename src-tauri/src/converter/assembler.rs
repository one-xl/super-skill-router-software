use super::matcher::ScoredSkill;
use super::parser::Scenario;

pub fn assemble(requirement: &str, scenario: Scenario, skills: &[ScoredSkill]) -> String {
    let template = match scenario {
        Scenario::Coding => include_str!("templates/coding.md"),
        Scenario::Refactor => include_str!("templates/refactor.md"),
        Scenario::Debug => include_str!("templates/debug.md"),
        Scenario::Review => include_str!("templates/review.md"),
        Scenario::Generic => include_str!("templates/generic.md"),
    };
    let goal = requirement
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("完成用户需求")
        .trim();
    let context = "遵循需求中给出的项目类型、技术栈和约束；信息不足时先说明必要假设。";
    let skill_lines = if skills.is_empty() {
        "- 未检测到相关且已安装的 skill。".into()
    } else {
        skills
            .iter()
            .map(|item| {
                let usage = if item.skill.when_to_use.trim().is_empty() {
                    item.skill.description.as_str()
                } else {
                    item.skill.when_to_use.as_str()
                };
                format!(
                    "- `{}` — {}，在 {} 时使用",
                    item.skill.name,
                    compact(&item.skill.description),
                    compact(usage)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    template
        .replace("{{goal}}", goal)
        .replace("{{context}}", context)
        .replace("{{skills}}", &skill_lines)
}

fn compact(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(180)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converter::matcher::{ScoredSkill, SkillMetadata};

    #[test]
    fn assembles_the_required_sections_using_only_skill_metadata() {
        let skill = ScoredSkill {
            skill: SkillMetadata {
                id: "pdf".into(),
                name: "pdf-toolkit".into(),
                description: "Create and inspect PDF documents".into(),
                when_to_use: "when the task needs PDF output".into(),
                tags: vec!["pdf".into()],
                frecency: 0.0,
            },
            score: 5.0,
        };

        let prompt = assemble("Create a PDF report", Scenario::Coding, &[skill]);
        for heading in [
            "## 目标",
            "## 上下文",
            "## 可调用的 Skill（已检测到本机已安装）",
            "## 执行要求",
            "## 交付标准",
            "## 边界",
        ] {
            assert!(prompt.contains(heading), "missing heading: {heading}");
        }
        assert!(prompt.contains("`pdf-toolkit`"));
        assert!(prompt.contains("Create and inspect PDF documents"));
        assert!(prompt.contains("when the task needs PDF output"));
    }
}
