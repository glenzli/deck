//! Bounded production-script grammar. All controls are parsed before any synthesis.
use super::{
    CueLevel, MAX_SCRIPT_EVENTS, SPEECH_SCRIPT_REVISION, SpeechCueAction, SpeechDelivery,
    SpeechProduction, SpeechRole, SpeechScriptEvent as Event, SpeechScriptEventKind as Kind,
    SpeechScriptIssue, SpeechScriptPlan, valid_label,
};
use crate::ContentDigest;
use std::collections::BTreeMap;

/// Parses exact source bytes; repeated audio is represented as control events, not duplicate text.
#[must_use]
pub fn parse_speech_script(source: &str) -> SpeechScriptPlan {
    parse_speech_script_for_review(source).0
}

/// Returns the unchanged execution plan plus source lines of authored, non-spoken notes.
/// Role/language control events retain their historical Note encoding but are not review notes.
#[must_use]
pub fn parse_speech_script_for_review(source: &str) -> (SpeechScriptPlan, Vec<u32>) {
    let mut parser = Parser {
        plan: SpeechScriptPlan {
            revision: SPEECH_SCRIPT_REVISION.into(),
            source_digest: ContentDigest::from_bytes(source.as_bytes()),
            production: SpeechProduction::default(),
            delivery: SpeechDelivery::default(),
            roles: BTreeMap::new(),
            cues: BTreeMap::new(),
            events: Vec::new(),
            issues: Vec::new(),
        },
        note_lines: Vec::new(),
        role: String::new(),
        language: None,
        delivery: None,
        repeat: None,
        body: false,
        production_set: false,
        delivery_set: false,
    };
    if source.len() > 65_536 {
        parser.issue(1, "script_too_large", "");
        return (parser.plan, parser.note_lines);
    }
    for (index, line) in source.lines().enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if parser.plan.events.len() >= MAX_SCRIPT_EVENTS {
            parser.issue(number, "too_many_events", "");
            break;
        }
        parser.line(number, line.trim());
    }
    if let Some(repeat) = &parser.repeat {
        parser.issue(repeat.line, "unclosed_repeat", "");
    }
    if !parser
        .plan
        .events
        .iter()
        .any(|e| matches!(e.kind, Kind::Speech { .. }))
    {
        parser.issue(1, "no_spoken_text", "");
    }
    (parser.plan, parser.note_lines)
}
struct RepeatScope {
    line: u32,
    start: usize,
    role: String,
    language: Option<String>,
    delivery: Option<SpeechDelivery>,
}
struct Parser {
    plan: SpeechScriptPlan,
    note_lines: Vec<u32>,
    role: String,
    language: Option<String>,
    delivery: Option<SpeechDelivery>,
    repeat: Option<RepeatScope>,
    body: bool,
    production_set: bool,
    delivery_set: bool,
}
impl Parser {
    fn emit(&mut self, line: u32, kind: Kind) {
        self.plan.events.push(Event { line, kind });
    }
    fn issue(&mut self, line: u32, code: &str, text: &str) {
        if self.plan.issues.len() < 128 {
            self.plan.issues.push(SpeechScriptIssue {
                line,
                code: code.into(),
                text: text.chars().take(160).collect(),
            });
        }
    }
    fn line(&mut self, number: u32, text: &str) {
        if text.is_empty() {
            return;
        }
        if text.starts_with("```")
            || text.starts_with("~~~")
            || text.starts_with('|')
            || text.contains("![")
            || text.contains('<')
        {
            self.issue(number, "unsupported_markup", text);
            return;
        }
        let heading = text.trim_start_matches('#');
        if heading.len() != text.len() && heading.starts_with(char::is_whitespace) {
            self.emit(
                number,
                Kind::Heading {
                    text: clean_inline(heading.trim()),
                },
            );
            return;
        }
        if matches!(text, "---" | "***" | "___") {
            return;
        }
        if text.starts_with('[')
            && text.ends_with(']')
            && !text[1..text.len() - 1].contains(['[', ']'])
        {
            self.directive(number, &text[1..text.len() - 1]);
        } else if text
            .char_indices()
            .any(|(i, c)| c == '[' && (i == 0 || text.as_bytes()[i - 1] != b'\\'))
        {
            self.issue(number, "directive_line", text);
        } else {
            let text = clean_inline(text);
            if !text.is_empty() {
                self.body = true;
                let role = self.plan.roles.get(&self.role);
                if !self.role.is_empty() && role.is_none() {
                    self.issue(number, "undeclared_role", &self.role.clone());
                    return;
                }
                if self.role.is_empty() && !self.plan.roles.is_empty() {
                    self.issue(number, "missing_speaker", "");
                    return;
                }
                let role = self.plan.roles.get(&self.role);
                let language = self
                    .language
                    .clone()
                    .or_else(|| role.map(|r| r.language.clone()));
                let delivery = self
                    .delivery
                    .or_else(|| role.and_then(|r| r.delivery))
                    .unwrap_or(self.plan.delivery);
                self.emit(
                    number,
                    Kind::Speech {
                        role: self.role.clone(),
                        language,
                        delivery,
                        text,
                    },
                );
            }
        }
    }
    // One exhaustive grammar dispatch keeps command acceptance auditable.
    #[allow(clippy::too_many_lines)]
    fn directive(&mut self, line: u32, directive: &str) {
        if matches!(directive.trim(), "end-repeat" | "结束重复") {
            self.end_repeat(line);
            return;
        }
        let Some((name, value)) = directive.split_once([':', '：']) else {
            self.issue(line, "unknown_directive", directive);
            return;
        };
        let value = value.trim();
        match name.trim() {
            "production" | "用途" => {
                if self.body || self.production_set {
                    self.issue(line, "declaration_position", directive);
                    return;
                }
                if let Some(value) = SpeechProduction::parse(value) {
                    self.plan.production = value;
                    self.production_set = true;
                } else {
                    self.issue(line, "invalid_production", directive);
                }
            }
            "delivery" | "整体语气" => {
                if self.body || self.delivery_set {
                    self.issue(line, "declaration_position", directive);
                    return;
                }
                if let Some(value) = SpeechDelivery::parse(value) {
                    self.plan.delivery = value;
                    self.delivery_set = true;
                } else {
                    self.issue(line, "invalid_delivery", directive);
                }
            }
            "role" | "定义角色" => self.declare_role(line, value),
            "cue" | "定义提示音" => self.declare_cue(line, value),
            "repeat" | "重复" => self.begin_repeat(line, value),
            "scene" | "片段" if valid_label(value) => {
                if self.repeat.is_some() {
                    self.issue(line, "scene_inside_repeat", directive);
                    return;
                }
                self.body = true;
                self.role.clear();
                self.language = None;
                self.delivery = None;
                self.emit(
                    line,
                    Kind::Scene {
                        label: value.into(),
                    },
                );
            }
            "停顿" | "pause" => match pause_millis(value) {
                Some(milliseconds) => {
                    self.body = true;
                    self.emit(line, Kind::Pause { milliseconds });
                }
                None => self.issue(line, "invalid_pause", directive),
            },
            "角色" | "speaker" if valid_label(value) => {
                if !self.plan.roles.contains_key(value) {
                    self.issue(line, "undeclared_role", value);
                }
                self.role = value.into();
                // Language overrides are scoped to a role selection; bilingual roles normally use auto.
                self.language = None;
                self.delivery = None;
                self.emit(
                    line,
                    Kind::Note {
                        text: directive.into(),
                    },
                );
            }
            "语言" | "language" => match language(value) {
                Some(value) => {
                    self.language = Some(value.into());
                    self.emit(
                        line,
                        Kind::Note {
                            text: directive.into(),
                        },
                    );
                }
                None => self.issue(line, "unsupported_language", directive),
            },
            "performance" | "局部语气" => {
                if matches!(value, "default" | "默认") {
                    self.delivery = None;
                } else if let Some(value) = SpeechDelivery::parse(value) {
                    self.delivery = Some(value);
                } else {
                    self.issue(line, "invalid_delivery", directive);
                }
            }
            "音效" | "audio" | "提示音" if valid_label(value) => {
                self.body = true;
                if !self.plan.cues.contains_key(value) {
                    self.issue(line, "undeclared_cue", value);
                }
                self.emit(
                    line,
                    Kind::Cue {
                        label: value.into(),
                    },
                );
            }
            "备注" | "note" => {
                self.note_lines.push(line);
                self.emit(line, Kind::Note { text: value.into() });
            }
            _ => self.issue(line, "unknown_directive", directive),
        }
    }
    fn declare_role(&mut self, line: u32, value: &str) {
        let Some((label, mut fields)) = fields(value) else {
            self.issue(line, "invalid_role", value);
            return;
        };
        if self.body {
            self.issue(line, "declaration_position", value);
            return;
        }
        let language = fields
            .remove("language")
            .or_else(|| fields.remove("语言"))
            .unwrap_or("auto");
        let delivery = fields.remove("delivery").or_else(|| fields.remove("语气"));
        let description = fields
            .remove("description")
            .or_else(|| fields.remove("说明"))
            .unwrap_or("");
        if !valid_label(label)
            || self::language(language).is_none()
            || description.len() > 256
            || !fields.is_empty()
            || delivery.is_some_and(|v| SpeechDelivery::parse(v).is_none())
            || self.plan.roles.len() >= 32
            || self.plan.roles.contains_key(label)
        {
            self.issue(line, "invalid_role", value);
            return;
        }
        let delivery = delivery.and_then(SpeechDelivery::parse);
        self.plan.roles.insert(
            label.into(),
            SpeechRole {
                language: self::language(language).unwrap().into(),
                delivery,
                description: description.into(),
            },
        );
    }
    fn declare_cue(&mut self, line: u32, value: &str) {
        let Some((label, mut fields)) = fields(value) else {
            self.issue(line, "invalid_cue", value);
            return;
        };
        if self.body {
            self.issue(line, "declaration_position", value);
            return;
        }
        let sound = fields
            .remove("sound")
            .or_else(|| fields.remove("声音"))
            .unwrap_or("external");
        let action = match sound {
            "beep" | "短音" => {
                let duration = fields
                    .remove("duration")
                    .or_else(|| fields.remove("时长"))
                    .unwrap_or("0.2s");
                let level = fields
                    .remove("level")
                    .or_else(|| fields.remove("音量"))
                    .unwrap_or("soft");
                let milliseconds = pause_millis(duration);
                if !milliseconds.is_some_and(|ms| (20..=2000).contains(&ms))
                    || !matches!(level, "soft" | "normal" | "轻柔" | "标准")
                {
                    self.issue(line, "invalid_cue", value);
                    return;
                }
                Some(SpeechCueAction::Beep {
                    milliseconds: milliseconds.unwrap(),
                    level: if matches!(level, "soft" | "轻柔") {
                        CueLevel::Soft
                    } else {
                        CueLevel::Normal
                    },
                })
            }
            "chime" | "叮咚" => Some(SpeechCueAction::Chime),
            "external" | "文件" => None,
            _ => {
                self.issue(line, "invalid_cue", value);
                return;
            }
        };
        if !valid_label(label)
            || !fields.is_empty()
            || self.plan.cues.contains_key(label)
            || self.plan.cues.len() >= 128
        {
            self.issue(line, "invalid_cue", value);
            return;
        }
        self.plan.cues.insert(label.into(), action);
    }
    fn begin_repeat(&mut self, line: u32, value: &str) {
        if self.repeat.is_some() {
            self.issue(line, "nested_repeat", value);
            return;
        }
        let Some((count, mut fields)) = fields(value) else {
            self.issue(line, "invalid_repeat", value);
            return;
        };
        let count = count.trim_end_matches('次').trim().parse::<u8>().ok();
        let gap = fields
            .remove("gap")
            .or_else(|| fields.remove("间隔"))
            .unwrap_or("0s");
        let gap_ms = if matches!(gap, "0" | "0s" | "0秒") {
            Some(0)
        } else {
            pause_millis(gap)
        };
        let between_cue = fields.remove("cue").or_else(|| fields.remove("提示音"));
        if between_cue
            .is_some_and(|label| !valid_label(label) || !self.plan.cues.contains_key(label))
        {
            self.issue(line, "undeclared_cue", value);
            return;
        }
        if !count.is_some_and(|c| (1..=8).contains(&c))
            || (count == Some(1) && between_cue.is_some())
            || gap_ms.is_none()
            || !fields.is_empty()
        {
            self.issue(line, "invalid_repeat", value);
            return;
        }
        self.body = true;
        self.repeat = Some(RepeatScope {
            line,
            start: self.plan.events.len(),
            role: self.role.clone(),
            language: self.language.clone(),
            delivery: self.delivery,
        });
        self.emit(
            line,
            Kind::RepeatStart {
                count: count.unwrap(),
                gap_ms: gap_ms.unwrap(),
                between_cue: between_cue.map(str::to_owned),
            },
        );
    }
    fn end_repeat(&mut self, line: u32) {
        let Some(scope) = self.repeat.take() else {
            self.issue(line, "unexpected_repeat_end", "");
            return;
        };
        if !self.plan.events[scope.start..]
            .iter()
            .any(|e| matches!(e.kind, Kind::Speech { .. }))
        {
            self.issue(line, "empty_repeat", "");
        }
        self.role = scope.role;
        self.language = scope.language;
        self.delivery = scope.delivery;
        self.emit(line, Kind::RepeatEnd);
    }
}
fn fields(value: &str) -> Option<(&str, BTreeMap<&str, &str>)> {
    let mut parts = value.split([';', '；']);
    let name = parts.next()?.trim();
    let mut fields = BTreeMap::new();
    for part in parts {
        let (key, value) = part.split_once([':', '：'])?;
        if fields.insert(key.trim(), value.trim()).is_some() {
            return None;
        }
    }
    Some((name, fields))
}
fn language(label: &str) -> Option<&'static str> {
    match label {
        "自动" | "auto" => Some("auto"),
        "中文" | "Chinese" => Some("Chinese"),
        "英语" | "英文" | "English" => Some("English"),
        "日语" | "Japanese" => Some("Japanese"),
        "韩语" | "Korean" => Some("Korean"),
        "德语" | "German" => Some("German"),
        "法语" | "French" => Some("French"),
        "俄语" | "Russian" => Some("Russian"),
        "葡萄牙语" | "Portuguese" => Some("Portuguese"),
        "西班牙语" | "Spanish" => Some("Spanish"),
        "意大利语" | "Italian" => Some("Italian"),
        _ => None,
    }
}
fn pause_millis(value: &str) -> Option<u32> {
    let value = value
        .strip_suffix("秒")
        .or_else(|| value.strip_suffix('s'))
        .unwrap_or(value)
        .trim();
    let (seconds, fraction) = value.split_once('.').unwrap_or((value, ""));
    if seconds.is_empty()
        || !seconds.bytes().all(|c| c.is_ascii_digit())
        || fraction.len() > 3
        || !fraction.bytes().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let millis = seconds
        .parse::<u32>()
        .ok()?
        .checked_mul(1000)?
        .checked_add(if fraction.is_empty() {
            0
        } else {
            fraction.parse::<u32>().ok()? * 10_u32.pow(3 - u32::try_from(fraction.len()).ok()?)
        })?;
    (millis > 0 && millis <= 120_000).then_some(millis)
}
fn clean_inline(text: &str) -> String {
    let text = text
        .replace("&#x20;", " ")
        .replace("&#32;", " ")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("**", "")
        .replace("__", "")
        .replace('`', "")
        .replace("\\[", "[")
        .replace("\\]", "]");
    text.trim().to_owned()
}
