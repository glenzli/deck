use super::*;

#[test]
fn bilingual_cover_title_author_and_body_map_to_storybook_roles() {
    for script in [
        "[Cover] Moon\n[Title] Night\n[Author] Ada\n[1] Hello\n[2] Goodnight",
        "[封面] 月亮\n[扉页] 夜晚\n[作者] 艾达\n[1] 你好\n[2] 晚安",
    ] {
        let parsed = parse(script, 4).unwrap();
        assert!(parsed.has_title);
        assert_eq!(
            parsed.page_text.keys().copied().collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        assert!(!parsed.author.is_empty());
    }
    let plain = parse("[Cover] Moon [1] Stars", 2).unwrap();
    assert!(!plain.has_title);
    assert_eq!(plain.page_text[&1], "Stars");
}

#[test]
fn silent_overwrites_discarded_preamble_and_out_of_range_pages_are_errors() {
    for script in [
        "lost text [Cover] title",
        "[Cover] A [封面] B",
        "[Cover] A [1] B [01] C",
        "[Cover] A [Author] A [作者] B",
        "[Cover] A [0] zero",
        "[Cover] A [2] absent",
        "[Cover] A [Title] T [1] shifted outside",
        "[Cover] A\n[Scene] typo",
        "[1] no cover",
    ] {
        assert!(parse(script, 2).is_err(), "{script}");
    }
}
