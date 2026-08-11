use std::time::Duration;

use serde::Deserialize;

const TRANSLATE_URL: &str = "https://api.mymemory.translated.net/get";
const MAX_CHUNK_BYTES: usize = 420;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateRequest {
    pub markdown: String,
}

#[tauri::command]
pub async fn translate_markdown(request: TranslateRequest) -> Result<String, String> {
    if request.markdown.trim().is_empty() {
        return Err("没有可翻译的 Markdown 内容。".into());
    }
    let client = reqwest::Client::builder()
        .user_agent("Super-Skill-Router/0.1")
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("无法初始化翻译服务：{error}"))?;
    translate_document(&client, &request.markdown).await
}

async fn translate_document(client: &reqwest::Client, markdown: &str) -> Result<String, String> {
    let mut output = String::with_capacity(markdown.len());
    let mut in_frontmatter = false;
    let mut in_code_block = false;

    for (index, source_line) in markdown.split_inclusive('\n').enumerate() {
        let (line, ending) = split_line_ending(source_line);
        let trimmed = line.trim();
        if index == 0 && trimmed == "---" {
            in_frontmatter = true;
            output.push_str(source_line);
            continue;
        }
        if in_frontmatter {
            output.push_str(source_line);
            if trimmed == "---" {
                in_frontmatter = false;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_block = !in_code_block;
            output.push_str(source_line);
            continue;
        }
        if in_code_block || !contains_english(line) {
            output.push_str(source_line);
            continue;
        }

        let (prefix, body) = markdown_prefix(line);
        output.push_str(prefix);
        output.push_str(&translate_text(client, body).await?);
        output.push_str(ending);
    }
    Ok(output)
}

async fn translate_text(client: &reqwest::Client, text: &str) -> Result<String, String> {
    let chunks = split_text_chunks(text, MAX_CHUNK_BYTES);
    let mut translated = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let mut url = reqwest::Url::parse(TRANSLATE_URL)
            .map_err(|error| format!("无法创建翻译地址：{error}"))?;
        url.query_pairs_mut()
            .append_pair("q", &chunk)
            .append_pair("langpair", "en|zh-CN");
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|error| format!("免费翻译服务暂时不可用：{error}"))?;
        let status = response.status();
        let payload: TranslationResponse = response
            .json()
            .await
            .map_err(|error| format!("免费翻译服务返回无法解析：{error}"))?;
        if !status.is_success()
            || payload.quota_finished.unwrap_or(false)
            || payload.response_status != 200
        {
            return Err(payload
                .response_details
                .unwrap_or_else(|| "免费翻译额度已用尽或服务暂时不可用，请稍后重试。".into()));
        }
        let value = payload.response_data.translated_text.trim().to_owned();
        if value.is_empty() {
            return Err("免费翻译服务没有返回有效内容。".into());
        }
        translated.push(value);
    }
    Ok(translated.join(" "))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranslationResponse {
    response_data: TranslationData,
    #[serde(deserialize_with = "deserialize_status")]
    response_status: u16,
    #[serde(default)]
    response_details: Option<String>,
    #[serde(default)]
    quota_finished: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranslationData {
    translated_text: String,
}

fn deserialize_status<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(number) => number
            .as_u64()
            .and_then(|number| u16::try_from(number).ok())
            .ok_or_else(|| serde::de::Error::custom("invalid numeric responseStatus")),
        serde_json::Value::String(value) => value
            .parse::<u16>()
            .map_err(|_| serde::de::Error::custom("invalid string responseStatus")),
        _ => Err(serde::de::Error::custom("invalid responseStatus")),
    }
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(value) = line.strip_suffix("\r\n") {
        (value, "\r\n")
    } else if let Some(value) = line.strip_suffix('\n') {
        (value, "\n")
    } else {
        (line, "")
    }
}

fn contains_english(value: &str) -> bool {
    value.bytes().any(|byte| byte.is_ascii_alphabetic())
}

fn markdown_prefix(line: &str) -> (&str, &str) {
    let leading = line.len() - line.trim_start().len();
    let rest = &line[leading..];
    let marker_length = if rest.starts_with("- [ ] ") || rest.starts_with("- [x] ") {
        6
    } else if rest.starts_with("- ")
        || rest.starts_with("* ")
        || rest.starts_with("+ ")
        || rest.starts_with("> ")
    {
        2
    } else if rest.starts_with('#') {
        rest.find(' ').map_or(0, |index| index + 1)
    } else {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && rest[digits..].starts_with(". ") {
            digits + 2
        } else {
            0
        }
    };
    let split = leading + marker_length;
    (&line[..split], &line[split..])
}

fn split_text_chunks(text: &str, max_bytes: usize) -> Vec<String> {
    if text.len() <= max_bytes {
        return vec![text.into()];
    }
    let mut chunks = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let separator = usize::from(!current.is_empty());
        if !current.is_empty() && current.len() + separator + word.len() > max_bytes {
            chunks.push(std::mem::take(&mut current));
        }
        if word.len() > max_bytes {
            for character in word.chars() {
                if current.len() + character.len_utf8() > max_bytes {
                    chunks.push(std::mem::take(&mut current));
                }
                current.push(character);
            }
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_common_markdown_prefixes() {
        assert_eq!(markdown_prefix("## Goal"), ("## ", "Goal"));
        assert_eq!(markdown_prefix("- [ ] Run tests"), ("- [ ] ", "Run tests"));
        assert_eq!(markdown_prefix("  12. Review"), ("  12. ", "Review"));
    }

    #[test]
    fn chunks_text_on_utf8_boundaries() {
        let chunks = split_text_chunks(&"word ".repeat(200), 64);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|chunk| chunk.len() <= 64));
    }

    #[test]
    fn detects_english_without_retranslating_chinese_only_lines() {
        assert!(contains_english("Use this skill"));
        assert!(!contains_english("使用这个技能"));
    }

    #[tokio::test]
    #[ignore = "calls the public MyMemory translation service"]
    async fn translates_markdown_and_preserves_code_blocks() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .expect("client");
        let translated = translate_document(
            &client,
            "## Goal\nTranslate this instruction.\n```bash\nnpm install\n```\n",
        )
        .await
        .expect("translation");
        assert!(translated.contains("## "));
        assert!(translated.contains("npm install"));
        assert_ne!(
            translated,
            "## Goal\nTranslate this instruction.\n```bash\nnpm install\n```\n"
        );
    }
}
