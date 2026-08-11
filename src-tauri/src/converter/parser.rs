#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    Coding,
    Refactor,
    Debug,
    Review,
    Generic,
}

pub fn classify(requirement: &str) -> Scenario {
    let value = requirement.to_lowercase();
    if ["重构", "refactor", "改造", "迁移"]
        .iter()
        .any(|keyword| value.contains(keyword))
    {
        Scenario::Refactor
    } else if ["排查", "修复", "报错", "bug", "debug", "故障"]
        .iter()
        .any(|keyword| value.contains(keyword))
    {
        Scenario::Debug
    } else if ["审查", "评审", "review", "检查代码", "code review"]
        .iter()
        .any(|keyword| value.contains(keyword))
    {
        Scenario::Review
    } else if ["开发", "实现", "编写", "构建", "代码", "coding", "build"]
        .iter()
        .any(|keyword| value.contains(keyword))
    {
        Scenario::Coding
    } else {
        Scenario::Generic
    }
}

pub fn keywords(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut ascii = String::new();
    let mut cjk = Vec::new();
    let flush_ascii = |buffer: &mut String, output: &mut Vec<String>| {
        if buffer.len() >= 2 {
            output.push(buffer.to_lowercase());
        }
        buffer.clear();
    };
    let flush_cjk = |buffer: &mut Vec<char>, output: &mut Vec<String>| {
        if buffer.len() >= 2 {
            if buffer.len() <= 12 {
                output.push(buffer.iter().collect());
            }
            for pair in buffer.windows(2) {
                output.push(pair.iter().collect());
            }
        }
        buffer.clear();
    };
    for character in input.chars() {
        if character.is_ascii_alphanumeric() {
            flush_cjk(&mut cjk, &mut words);
            ascii.push(character);
        } else if ('\u{4e00}'..='\u{9fff}').contains(&character) {
            flush_ascii(&mut ascii, &mut words);
            cjk.push(character);
        } else {
            flush_ascii(&mut ascii, &mut words);
            flush_cjk(&mut cjk, &mut words);
        }
    }
    flush_ascii(&mut ascii, &mut words);
    flush_cjk(&mut cjk, &mut words);
    words.sort();
    words.dedup();
    words
}
