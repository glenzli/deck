use super::*;

#[test]
fn real_chinese_rule_and_closing_combinations_keep_a_character_with_the_punctuation() {
    let rule = "获得 1 点专注，然后抽 1 张牌。";
    let wrapped = wrap(rule, 30);
    assert!(!wrapped.overflow);
    assert!(wrapped.lines.len() > 1);
    assert!(wrapped.lines.last().unwrap().ends_with("牌。"));
    assert!(
        wrapped
            .lines
            .iter()
            .all(|line| !line.starts_with(closing_punctuation))
    );
    assert_eq!(
        wrapped.lines.concat().replace(' ', ""),
        rule.replace(' ', "")
    );
    for tail in ["，", "。", "？！", "。”】", "……）》"] {
        let text = format!("{}{tail}", "文".repeat(17));
        let wrapped = wrap(&text, 30);
        assert!(!wrapped.overflow);
        assert_eq!(wrapped.lines.concat(), text);
        assert!(
            wrapped
                .lines
                .iter()
                .all(|line| !line.starts_with(closing_punctuation))
        );
        assert!(
            wrapped
                .lines
                .iter()
                .all(|line| line.chars().map(glyph_units).sum::<usize>() <= 638 * 4 / 30)
        );
    }
}

#[test]
fn authored_newline_is_preserved_and_an_unbreakable_punctuation_run_reports_overflow() {
    let authored = wrap("保留手动换行\n。下一段", 30);
    assert!(!authored.overflow);
    assert_eq!(authored.lines, ["保留手动换行", "。下一段"]);
    let source = format!("文{}", "。".repeat(40));
    let wrapped = wrap(&source, 30);
    assert!(wrapped.overflow);
    assert_eq!(wrapped.lines.concat(), source);
    assert!(
        wrapped
            .lines
            .iter()
            .all(|line| line.chars().map(glyph_units).sum::<usize>() <= 638 * 4 / 30)
    );
}

#[test]
fn flex_keeps_spring_and_fitting_ascii_words_whole_at_their_conservative_advance_boundary() {
    let spring = wrap_flex_to_width("SPRING\n春", 105.0, 690.0);
    assert_eq!(spring.lines, ["SPRING", "春"]);
    assert!(!spring.overflow);
    assert_eq!(
        wrap_flex_to_width("GO SPRING", 105.0, 690.0).lines,
        ["GO", "SPRING"]
    );
    for glyph in ['W', '@'] {
        let six = glyph.to_string().repeat(6);
        let exact = wrap_flex_to_width(&six, 100.0, 630.0);
        assert_eq!(exact.lines.as_slice(), std::slice::from_ref(&six));
        assert!(!exact.overflow);
        let tighter = wrap_flex_to_width(&six, 100.0, 629.0);
        assert_eq!(
            tighter.lines,
            [glyph.to_string().repeat(5), glyph.to_string()]
        );
        assert_eq!(tighter.lines.concat(), six);
    }
    // The legacy path retains its fixed 638-pixel copy area and original 1.25-em ASCII cost.
    assert_eq!(wrap("SPRING", 105).lines, ["SPRI", "NG"]);
}

#[test]
fn flex_keeps_cjk_closers_unicode_budgets_and_all_long_copy_characters() {
    let text = format!(
        "{}\n获得 1 点专注，然后抽 1 张牌。\n{}。”】",
        "W@".repeat(200),
        "文".repeat(40)
    );
    let wrapped = wrap_flex_to_width(&text, 105.0, 690.0);
    assert!(!wrapped.overflow);
    assert_eq!(
        wrapped.lines.concat().replace(' ', ""),
        text.replace([' ', '\n'], "")
    );
    assert!(
        wrapped
            .lines
            .iter()
            .all(|line| !line.starts_with(closing_punctuation))
    );
    assert!(
        wrapped
            .lines
            .iter()
            .all(|line| line.chars().map(flex_glyph_units).sum::<usize>() <= 131)
    );
    assert_eq!(wrap_flex_to_width("春🌱", 100.0, 325.0).lines, ["春🌱"]);
    assert_eq!(wrap_flex_to_width("春🌱", 100.0, 324.0).lines, ["春", "🌱"]);
    let cluster = format!("春{}", "。".repeat(40));
    let rejected = wrap_flex_to_width(&cluster, 105.0, 690.0);
    assert!(rejected.overflow);
    assert_eq!(rejected.lines.concat(), cluster);
}
