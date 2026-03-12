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

/// Find the closest matches for a query from a list of candidates.
/// Returns candidates with edit distance ≤ max_distance, sorted by distance.
pub fn suggest(query: &str, candidates: &[String], max_distance: usize) -> Vec<(String, usize)> {
    let mut matches: Vec<(String, usize)> = candidates
        .iter()
        .filter_map(|c| {
            // Check against full name and just the name part
            let dist_full = levenshtein(query, c);
            let name_part = c.rsplit('/').next().unwrap_or(c);
            let dist_name = levenshtein(query, name_part);
            let best = dist_full.min(dist_name);
            if best <= max_distance {
                Some((c.clone(), best))
            } else {
                None
            }
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
    fn test_suggest_sorted_by_distance() {
        let candidates: Vec<String> = vec![
            "org/abcdef".into(),
            "org/abcde".into(),
            "org/abcd".into(),
        ];
        let suggestions = suggest("abcde", &candidates, 3);
        assert!(suggestions.len() >= 2);
        // Exact match first
        assert!(suggestions[0].1 <= suggestions[1].1);
    }

    #[test]
    fn test_suggest_truncates_to_five() {
        let candidates: Vec<String> = (0..20).map(|i| format!("org/a{i}")).collect();
        let suggestions = suggest("a", &candidates, 5);
        assert!(suggestions.len() <= 5);
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;

    #[test]
    fn test_suggest_exact_match() {
        let names = vec!["hello".to_string(), "world".to_string(), "help".to_string()];
        let suggestions = suggest("hello", &names, 3);
        // Exact match should have distance 0
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].0, "hello");
    }

    #[test]
    fn test_suggest_empty_query() {
        let names = vec!["ab".to_string()];
        // Empty query with short candidate within distance 3
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
        // Only items within distance 1
        let suggestions = suggest("cat", &names, 1);
        assert!(suggestions.iter().all(|(_, d)| *d <= 1));
        assert!(suggestions.iter().any(|(n, _)| n == "cat"));
        assert!(suggestions.iter().any(|(n, _)| n == "bat"));
        assert!(!suggestions.iter().any(|(n, _)| n == "zzzzzzzzz"));
    }

    #[test]
    fn test_levenshtein_identical() {
        assert_eq!(levenshtein("abc", "abc"), 0);
    }

    #[test]
    fn test_levenshtein_empty() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
    }

    #[test]
    fn test_levenshtein_single_edit() {
        assert_eq!(levenshtein("cat", "bat"), 1);
        assert_eq!(levenshtein("cat", "cats"), 1);
        assert_eq!(levenshtein("cat", "at"), 1);
    }
}
