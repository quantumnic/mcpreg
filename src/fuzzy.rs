/// Compute Levenshtein distance between two strings (case-insensitive).
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[m][n]
}

/// Check if `needle` is a subsequence of `haystack` (case-insensitive).
/// E.g., "fs" is a subsequence of "filesystem".
pub fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let needle = needle.to_lowercase();
    let haystack = haystack.to_lowercase();
    let mut haystack_chars = haystack.chars();
    for ch in needle.chars() {
        loop {
            match haystack_chars.next() {
                Some(h) if h == ch => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

/// Check if `needle` is a substring of `haystack` (case-insensitive).
#[allow(dead_code)]
pub fn contains_substring(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// Compute a combined fuzzy score (lower is better, 0 = exact match).
/// Returns None if the candidate is too far from the query.
///
/// Scoring combines:
/// - Exact match → 0
/// - Substring match → 1
/// - Subsequence match → 2
/// - Levenshtein distance (capped at max_distance)
pub fn fuzzy_score(query: &str, candidate: &str, max_distance: usize) -> Option<usize> {
    let q = query.to_lowercase();
    let c = candidate.to_lowercase();

    if q == c {
        return Some(0);
    }
    // Prefix match ranks highest after exact match
    if c.starts_with(&q) {
        return Some(1);
    }
    // Non-prefix substring match
    if c.contains(&q) {
        return Some(2);
    }
    if is_subsequence(&q, &c) {
        return Some(3);
    }

    let dist = levenshtein(&q, &c);
    if dist <= max_distance {
        Some(3 + dist)
    } else {
        None
    }
}

/// Find the closest matches for a query from a list of candidates.
/// Uses combined fuzzy scoring: substring > subsequence > edit distance.
/// Returns candidates sorted by score (best first).
pub fn suggest(query: &str, candidates: &[String], max_distance: usize) -> Vec<(String, usize)> {
    let mut matches: Vec<(String, usize)> = candidates
        .iter()
        .filter_map(|c| {
            // Check against full name and just the name part (after last '/')
            let name_part = c.rsplit('/').next().unwrap_or(c);
            let score_full = fuzzy_score(query, c, max_distance);
            let score_name = fuzzy_score(query, name_part, max_distance);
            let best = match (score_full, score_name) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };
            best.map(|score| (c.clone(), score))
        })
        .collect();
    matches.sort_by_key(|(_, d)| *d);
    matches.truncate(5);
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_equal() {
        assert_eq!(levenshtein("hello", "hello"), 0);
    }

    #[test]
    fn test_levenshtein_empty() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
    }

    #[test]
    fn test_levenshtein_one_edit() {
        assert_eq!(levenshtein("cat", "bat"), 1);
        assert_eq!(levenshtein("cat", "cats"), 1);
        assert_eq!(levenshtein("cat", "at"), 1);
    }

    #[test]
    fn test_levenshtein_case_insensitive() {
        assert_eq!(levenshtein("Hello", "hello"), 0);
        assert_eq!(levenshtein("ABC", "abd"), 1);
    }

    #[test]
    fn test_is_subsequence() {
        assert!(is_subsequence("fs", "filesystem"));
        assert!(is_subsequence("fls", "filesystem"));
        assert!(is_subsequence("filesystem", "filesystem"));
        assert!(!is_subsequence("xyz", "filesystem"));
        assert!(is_subsequence("", "anything"));
    }

    #[test]
    fn test_contains_substring() {
        assert!(contains_substring("filesystem", "file"));
        assert!(contains_substring("filesystem", "system"));
        assert!(contains_substring("FileSystem", "file"));
        assert!(!contains_substring("filesystem", "xyz"));
    }

    #[test]
    fn test_fuzzy_score_exact() {
        assert_eq!(fuzzy_score("hello", "hello", 3), Some(0));
    }

    #[test]
    fn test_fuzzy_score_prefix() {
        // "file" is a prefix of "filesystem" → score = 1
        assert_eq!(fuzzy_score("file", "filesystem", 3), Some(1));
    }

    #[test]
    fn test_fuzzy_score_substring() {
        // "system" is a non-prefix substring → score = 2
        assert_eq!(fuzzy_score("system", "filesystem", 3), Some(2));
    }

    #[test]
    fn test_fuzzy_score_subsequence() {
        assert_eq!(fuzzy_score("fstm", "filesystem", 3), Some(3));
    }

    #[test]
    fn test_fuzzy_score_levenshtein() {
        // "filesytem" is a subsequence of "filesystem" → score = 3
        assert_eq!(fuzzy_score("filesytem", "filesystem", 3), Some(3));
        assert_eq!(fuzzy_score("xyzabc", "filesystem", 2), None);
    }

    #[test]
    fn test_fuzzy_score_prefix_beats_substring() {
        // Prefix should score better than non-prefix substring
        let prefix_score = fuzzy_score("file", "filesystem", 3).unwrap();
        let substring_score = fuzzy_score("system", "filesystem", 3).unwrap();
        assert!(prefix_score < substring_score, "Prefix should rank higher than substring");
    }

    #[test]
    fn test_fuzzy_score_too_far() {
        assert_eq!(fuzzy_score("zzzzz", "abc", 2), None);
    }

    #[test]
    fn test_suggest_finds_close_matches() {
        let candidates: Vec<String> = vec![
            "org/filesystem".into(),
            "org/sqlite".into(),
            "org/postgres".into(),
        ];
        let suggestions = suggest("filesytem", &candidates, 3);
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].0, "org/filesystem");
    }

    #[test]
    fn test_suggest_substring_beats_levenshtein() {
        let candidates: Vec<String> = vec![
            "org/web-search".into(),
            "org/websocket".into(),
        ];
        // "web" is a substring of both; should find both
        let suggestions = suggest("web", &candidates, 3);
        assert!(suggestions.len() == 2);
        // Both have substring score of 1
        assert!(suggestions[0].1 <= 1);
    }

    #[test]
    fn test_suggest_no_close_matches() {
        let candidates: Vec<String> = vec!["org/filesystem".into()];
        let suggestions = suggest("zzzzz", &candidates, 2);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_suggest_checks_name_part() {
        let candidates: Vec<String> = vec!["modelcontextprotocol/sqlite".into()];
        let suggestions = suggest("sqlit", &candidates, 2);
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].0, "modelcontextprotocol/sqlite");
    }

    #[test]
    fn test_suggest_sorted_by_score() {
        let candidates: Vec<String> = vec![
            "org/abcdef".into(),
            "org/abcde".into(),
            "org/abcd".into(),
        ];
        let suggestions = suggest("abcde", &candidates, 3);
        assert!(suggestions.len() >= 2);
        assert!(suggestions[0].1 <= suggestions[1].1);
    }

    #[test]
    fn test_suggest_truncates_to_five() {
        let candidates: Vec<String> = (0..20).map(|i| format!("org/a{i}")).collect();
        let suggestions = suggest("a", &candidates, 5);
        assert!(suggestions.len() <= 5);
    }

    #[test]
    fn test_suggest_exact_match() {
        let names = vec!["hello".to_string(), "world".to_string(), "help".to_string()];
        let suggestions = suggest("hello", &names, 3);
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].0, "hello");
        assert_eq!(suggestions[0].1, 0); // exact match
    }

    #[test]
    fn test_suggest_empty_query() {
        let names = vec!["ab".to_string()];
        let suggestions = suggest("", &names, 3);
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_suggest_empty_names() {
        let names: Vec<String> = vec![];
        let suggestions = suggest("hello", &names, 3);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_suggest_filters_by_distance() {
        let names = vec![
            "cat".to_string(),
            "bat".to_string(),
            "zzzzzzzzz".to_string(),
        ];
        let suggestions = suggest("cat", &names, 1);
        assert!(!suggestions.iter().any(|(n, _)| n == "zzzzzzzzz"));
    }

    #[test]
    fn test_levenshtein_identical() {
        assert_eq!(levenshtein("abc", "abc"), 0);
    }

    #[test]
    fn test_levenshtein_single_edit() {
        assert_eq!(levenshtein("cat", "bat"), 1);
        assert_eq!(levenshtein("cat", "cats"), 1);
        assert_eq!(levenshtein("cat", "at"), 1);
    }
}
