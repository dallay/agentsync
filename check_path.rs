use std::path::Path;
fn main() {
    let paths = vec![
        "C:/absolute/path/SKILL.md",
        "/absolute/path/SKILL.md",
        "../traversal/SKILL.md",
        "valid/skill/SKILL.md"
    ];
    for p_str in paths {
        let p = Path::new(p_str);
        let is_suspicious = p_str.contains("..") || p.is_absolute() || p_str.starts_with('/') || (p_str.len() > 1 && p_str.chars().nth(1) == Some(':'));
        println!("Path: {}, is_absolute: {}, starts_with_slash: {}, suspicious: {}", p_str, p.is_absolute(), p_str.starts_with('/'), is_suspicious);
    }
}
