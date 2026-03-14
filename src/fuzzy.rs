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

/// Compute Jaro similarity between two strings (case-insensitive).
/// Returns a value between 0.0 (no similarity) and 1.0 (identical).
pub fn jaro_similarity(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();

    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let match_distance = (a.len().max(b.len()) / 2).saturating_sub(1);

    let mut a_matched = vec![false; a.len()];
    let mut b_matched = vec![false; b.len()];
    let mut matches = 0.0_f64;
    let mut transpositions = 0.0_f64;

    for (i, &ac) in a.iter().enumerate() {
        let start = i.saturating_sub(match_distance);
        let end = (i + match_distance + 1).min(b.len());
        for j in start..end {
            if !b_matched[j] && ac == b[j] {
                a_matched[i] = true;
                b_matched[j] = true;
                matches += 1.0;
                break;
            }
        }
    }

    if matches == 0.0 {
        return 0.0;
    }

    let mut k = 0;
    for (i, &matched) in a_matched.iter().enumerate() {
        if !matched {
            continue;
        }
        while !b_matched[k] {
            k += 1;
        }
        if a[i] != b[k] {
            transpositions += 1.0;
        }
        k += 1;
    }

    (matches / a.len() as f64
        + matches / b.len() as f64
        + (matches - transpositions / 2.0) / matches)
        / 3.0
}

/// Compute Jaro-Winkler similarity (case-insensitive).
/// Boosts score for common prefixes. Returns 0.0..1.0.
pub fn jaro_winkler(a: &str, b: &str) -> f64 {
    let jaro = jaro_similarity(a, b);
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    let prefix_len = a_lower
        .chars()
        .zip(b_lower.chars())
        .take(4)
        .take_while(|(ac, bc)| ac == bc)
        .count();
    jaro + (prefix_len as f64 * 0.1 * (1.0 - jaro))
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
    // Acronym match (e.g., "fs" matches "file_system")
    if acronym_match(&q, &c) {
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
/// Uses combined fuzzy scoring: substring > subsequence > edit distance,
/// with a Jaro-Winkler fallback for candidates that are close but exceed edit distance.
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
                (None, None) => {
                    // Jaro-Winkler fallback: include if similarity >= 0.8
                    let jw_full = jaro_winkler(query, c);
                    let jw_name = jaro_winkler(query, name_part);
                    let jw = jw_full.max(jw_name);
                    if jw >= 0.8 {
                        // Convert similarity to a score (higher similarity = lower score)
                        Some(max_distance + 1 + ((1.0 - jw) * 10.0) as usize)
                    } else {
                        None
                    }
                }
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

    #[test]
    fn test_jaro_similarity_identical() {
        let s = jaro_similarity("hello", "hello");
        assert!((s - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_jaro_similarity_empty() {
        assert!((jaro_similarity("", "") - 1.0).abs() < f64::EPSILON);
        assert!((jaro_similarity("a", "")).abs() < f64::EPSILON);
    }

    #[test]
    fn test_jaro_similarity_different() {
        let s = jaro_similarity("abc", "xyz");
        assert!(s < 0.5, "Very different strings should have low similarity");
    }

    #[test]
    fn test_jaro_similarity_similar() {
        let s = jaro_similarity("filesystem", "filesytem");
        assert!(s > 0.9, "One-char typo should have high similarity: {s}");
    }

    #[test]
    fn test_jaro_winkler_boosts_prefix() {
        let jaro = jaro_similarity("filesystem", "filesytem");
        let jw = jaro_winkler("filesystem", "filesytem");
        assert!(jw >= jaro, "Jaro-Winkler should be >= Jaro for shared prefix");
    }

    #[test]
    fn test_jaro_winkler_identical() {
        let jw = jaro_winkler("hello", "hello");
        assert!((jw - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_jaro_winkler_case_insensitive() {
        let jw = jaro_winkler("Hello", "hello");
        assert!((jw - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_suggest_jaro_winkler_fallback() {
        let candidates: Vec<String> = vec![
            "org/postgresql-server".into(),
            "org/something-else".into(),
        ];
        // "postgresql" vs "postgresql-server" - name part has good JW similarity
        let suggestions = suggest("postgresql", &candidates, 2);
        assert!(!suggestions.is_empty(), "JW fallback should find postgresql-server");
    }
}

/// Compute normalized Levenshtein similarity (0.0 to 1.0, 1.0 = identical).
#[allow(dead_code)]
pub fn normalized_levenshtein(a: &str, b: &str) -> f64 {
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 1.0;
    }
    let dist = levenshtein(a, b) as f64;
    1.0 - dist / max_len as f64
}

/// Check if the query matches using acronym style.
/// E.g., "fs" matches "file_system", "mcp" matches "model_context_protocol".
pub fn acronym_match(query: &str, candidate: &str) -> bool {
    let query = query.to_lowercase();
    let candidate = candidate.to_lowercase();

    // Extract first letters of words (split on _, -, space)
    let acronym: String = candidate
        .split(['_', '-', ' ', '/'])
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.chars().next())
        .collect();

    acronym.contains(&query)
}

#[cfg(test)]
mod extended_tests {
    use super::*;

    #[test]
    fn test_normalized_levenshtein_identical() {
        let nl = normalized_levenshtein("hello", "hello");
        assert!((nl - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalized_levenshtein_empty() {
        let nl = normalized_levenshtein("", "");
        assert!((nl - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalized_levenshtein_one_empty() {
        let nl = normalized_levenshtein("abc", "");
        assert!((nl - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalized_levenshtein_one_edit() {
        let nl = normalized_levenshtein("cat", "bat");
        // distance 1, max_len 3 → 1 - 1/3 ≈ 0.666
        assert!((nl - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_normalized_levenshtein_range() {
        let nl = normalized_levenshtein("hello", "world");
        assert!((0.0..=1.0).contains(&nl), "Should be in [0, 1]: {nl}");
    }

    #[test]
    fn test_acronym_match_basic() {
        assert!(acronym_match("fs", "file_system"));
        assert!(acronym_match("mcp", "model_context_protocol"));
        assert!(acronym_match("ws", "web-search"));
    }

    #[test]
    fn test_acronym_match_slash() {
        assert!(acronym_match("mf", "modelcontextprotocol/filesystem"));
    }

    #[test]
    fn test_acronym_match_no_match() {
        assert!(!acronym_match("xyz", "file_system"));
    }

    #[test]
    fn test_acronym_match_case_insensitive() {
        assert!(acronym_match("FS", "File_System"));
    }

    #[test]
    fn test_acronym_match_single_word() {
        assert!(acronym_match("s", "sqlite"));
    }

    #[test]
    fn test_levenshtein_unicode() {
        // Unicode chars
        assert_eq!(levenshtein("café", "cafe"), 1);
        assert_eq!(levenshtein("über", "uber"), 1);
    }

    #[test]
    fn test_jaro_winkler_no_prefix() {
        // Strings that share no prefix
        let jw = jaro_winkler("abc", "xyz");
        let jaro = jaro_similarity("abc", "xyz");
        assert_eq!(jw, jaro, "No common prefix → JW == Jaro");
    }

    #[test]
    fn test_fuzzy_score_case_insensitive() {
        assert_eq!(fuzzy_score("FILE", "filesystem", 3), Some(1));
        assert_eq!(fuzzy_score("FileSystem", "filesystem", 3), Some(0));
    }

    #[test]
    fn test_suggest_prefers_shorter_match() {
        let candidates = vec![
            "org/sql".into(),
            "org/sqlite-advanced-tooling".into(),
        ];
        let suggestions = suggest("sql", &candidates, 3);
        assert!(!suggestions.is_empty());
        // "org/sql" should rank first (exact name match)
        assert_eq!(suggestions[0].0, "org/sql");
    }

    #[test]
    fn test_is_subsequence_empty_needle() {
        assert!(is_subsequence("", "anything"));
    }

    #[test]
    fn test_is_subsequence_equal() {
        assert!(is_subsequence("abc", "abc"));
    }

    #[test]
    fn test_is_subsequence_longer_needle() {
        assert!(!is_subsequence("abcdef", "abc"));
    }
}

#[allow(dead_code)]
/// Compute bigram (character pair) similarity between two strings.
/// Returns a value between 0.0 (no common bigrams) and 1.0 (identical bigrams).
/// Bigrams are less sensitive to character transpositions than Levenshtein.
pub fn bigram_similarity(a: &str, b: &str) -> f64 {
    let a = a.to_lowercase();
    let b = b.to_lowercase();

    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    if a_chars.len() < 2 && b_chars.len() < 2 {
        return if a == b { 1.0 } else { 0.0 };
    }
    if a_chars.len() < 2 || b_chars.len() < 2 {
        return 0.0;
    }

    let a_bigrams: Vec<(char, char)> = a_chars.windows(2).map(|w| (w[0], w[1])).collect();
    let b_bigrams: Vec<(char, char)> = b_chars.windows(2).map(|w| (w[0], w[1])).collect();

    let mut b_used = vec![false; b_bigrams.len()];
    let mut matches = 0usize;

    for ab in &a_bigrams {
        for (j, bb) in b_bigrams.iter().enumerate() {
            if !b_used[j] && ab == bb {
                b_used[j] = true;
                matches += 1;
                break;
            }
        }
    }

    let total = a_bigrams.len() + b_bigrams.len();
    if total == 0 {
        return 1.0;
    }
    (2 * matches) as f64 / total as f64
}

#[allow(dead_code)]
/// Combined similarity score using multiple algorithms.
/// Returns a value between 0.0 (completely different) and 1.0 (identical).
/// Combines Jaro-Winkler (40%), bigram (30%), and normalized Levenshtein (30%).
pub fn combined_similarity(a: &str, b: &str) -> f64 {
    let jw = jaro_winkler(a, b);
    let bg = bigram_similarity(a, b);
    let nl = normalized_levenshtein(a, b);
    jw * 0.4 + bg * 0.3 + nl * 0.3
}

#[allow(dead_code)]
/// Find the best matches using combined similarity scoring.
/// Returns candidates with similarity >= threshold, sorted by similarity (best first).
pub fn best_matches(
    query: &str,
    candidates: &[String],
    threshold: f64,
    max_results: usize,
) -> Vec<(String, f64)> {
    let mut scored: Vec<(String, f64)> = candidates
        .iter()
        .filter_map(|c| {
            let name_part = c.rsplit('/').next().unwrap_or(c);
            let sim_full = combined_similarity(query, c);
            let sim_name = combined_similarity(query, name_part);
            let sim = sim_full.max(sim_name);
            if sim >= threshold {
                Some((c.clone(), sim))
            } else {
                None
            }
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(max_results);
    scored
}

#[allow(dead_code)]
/// Tokenize a query string into searchable tokens.
/// Splits on whitespace, underscores, hyphens, and slashes.
pub fn tokenize(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split(|c: char| c.is_whitespace() || c == '_' || c == '-' || c == '/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

#[allow(dead_code)]
/// Check if all tokens from the query appear in the candidate text.
/// Useful for multi-word search queries.
pub fn all_tokens_match(query: &str, candidate: &str) -> bool {
    let tokens = tokenize(query);
    let candidate_lower = candidate.to_lowercase();
    tokens.iter().all(|t| candidate_lower.contains(t))
}

#[cfg(test)]
mod bigram_tests {
    use super::*;

    #[test]
    fn test_bigram_identical() {
        let s = bigram_similarity("hello", "hello");
        assert!((s - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bigram_empty() {
        assert!((bigram_similarity("", "") - 1.0).abs() < f64::EPSILON);
        assert!((bigram_similarity("a", "b") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bigram_similar() {
        let s = bigram_similarity("filesystem", "filesytem");
        assert!(s > 0.7, "One missing char should still be similar: {s}");
    }

    #[test]
    fn test_bigram_different() {
        let s = bigram_similarity("abc", "xyz");
        assert!(s < 0.1, "Completely different strings: {s}");
    }

    #[test]
    fn test_bigram_case_insensitive() {
        let s = bigram_similarity("Hello", "hello");
        assert!((s - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bigram_transposition() {
        // Transpositions should have higher bigram similarity than substitutions
        let trans = bigram_similarity("ab", "ba");
        let subst = bigram_similarity("ab", "cd");
        assert!(trans >= subst, "Transposition should score >= substitution");
    }

    #[test]
    fn test_combined_similarity_identical() {
        let s = combined_similarity("test", "test");
        assert!((s - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_combined_similarity_typo() {
        let s = combined_similarity("filesystem", "filesytem");
        assert!(s > 0.8, "One-char typo should have high combined similarity: {s}");
    }

    #[test]
    fn test_combined_similarity_different() {
        let s = combined_similarity("abc", "xyz");
        assert!(s < 0.3, "Very different strings should have low similarity: {s}");
    }

    #[test]
    fn test_best_matches_finds_close() {
        let candidates = vec![
            "org/filesystem".into(),
            "org/sqlite".into(),
            "org/postgres".into(),
        ];
        let matches = best_matches("filesytem", &candidates, 0.5, 5);
        assert!(!matches.is_empty());
        assert_eq!(matches[0].0, "org/filesystem");
    }

    #[test]
    fn test_best_matches_respects_threshold() {
        let candidates = vec!["org/abc".into(), "org/xyz".into()];
        let matches = best_matches("hello", &candidates, 0.9, 5);
        assert!(matches.is_empty(), "Nothing should match at 0.9 threshold");
    }

    #[test]
    fn test_best_matches_respects_max() {
        let candidates: Vec<String> = (0..20).map(|i| format!("org/tool-{i}")).collect();
        let matches = best_matches("tool", &candidates, 0.3, 3);
        assert!(matches.len() <= 3);
    }

    #[test]
    fn test_tokenize_basic() {
        assert_eq!(tokenize("hello world"), vec!["hello", "world"]);
    }

    #[test]
    fn test_tokenize_separators() {
        assert_eq!(tokenize("file_system-server/v2"), vec!["file", "system", "server", "v2"]);
    }

    #[test]
    fn test_tokenize_empty() {
        assert!(tokenize("").is_empty());
        assert!(tokenize("   ").is_empty());
    }

    #[test]
    fn test_all_tokens_match_basic() {
        assert!(all_tokens_match("file system", "MCP filesystem server"));
        assert!(!all_tokens_match("file database", "MCP filesystem server"));
    }

    #[test]
    fn test_all_tokens_match_single() {
        assert!(all_tokens_match("sql", "PostgreSQL database server"));
    }

    #[test]
    fn test_all_tokens_match_empty_query() {
        assert!(all_tokens_match("", "anything"));
    }

    #[test]
    fn test_bigram_single_char_strings() {
        // Single char strings have no bigrams
        assert!((bigram_similarity("a", "a") - 1.0).abs() < f64::EPSILON);
        assert!((bigram_similarity("a", "b") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bigram_two_char_strings() {
        assert!((bigram_similarity("ab", "ab") - 1.0).abs() < f64::EPSILON);
        assert!((bigram_similarity("ab", "cd") - 0.0).abs() < f64::EPSILON);
    }
}

#[cfg(test)]
mod additional_fuzzy_tests {
    use super::*;

    #[test]
    fn test_suggest_empty_haystack() {
        let candidates: Vec<String> = vec![];
        let results = suggest("test", &candidates, 3);
        assert!(results.is_empty());
    }

    #[test]
    fn test_best_matches_empty_query() {
        let candidates = vec!["hello".to_string(), "world".to_string()];
        let results = best_matches("", &candidates, 0.0, 5);
        // Just verify no panic
        let _ = results;
    }

    #[test]
    fn test_combined_similarity_exact() {
        let score = combined_similarity("filesystem", "filesystem");
        assert!(score > 0.99, "Exact match should have near-perfect score, got {score}");
    }

    #[test]
    fn test_combined_similarity_partial() {
        let score = combined_similarity("file", "filesystem");
        assert!(score > 0.0, "Partial match should have positive score");
        assert!(score < 1.0, "Partial match should not be perfect");
    }

    #[test]
    fn test_best_matches_limit_respected() {
        let candidates: Vec<String> = (0..100).map(|i| format!("server{i}")).collect();
        let results = best_matches("server", &candidates, 0.0, 3);
        assert!(results.len() <= 3);
    }

    #[test]
    fn test_bigram_similarity_identical() {
        let score = bigram_similarity("hello", "hello");
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bigram_similarity_different() {
        let score = bigram_similarity("abc", "xyz");
        assert!(score < 0.5, "Completely different strings should have low similarity");
    }

    #[test]
    fn test_acronym_match_positive() {
        assert!(acronym_match("fs", "file-system"));
    }

    #[test]
    fn test_acronym_match_negative() {
        assert!(!acronym_match("xyz", "file-system"));
    }

    #[test]
    fn test_jaro_winkler_identical() {
        let score = jaro_winkler("test", "test");
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_jaro_winkler_different() {
        let score = jaro_winkler("abc", "xyz");
        assert!(score < 0.5);
    }

    #[test]
    fn test_normalized_levenshtein_identical() {
        let score = normalized_levenshtein("hello", "hello");
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalized_levenshtein_empty() {
        let score = normalized_levenshtein("", "");
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tokenize_splits_correctly() {
        let tokens = tokenize("hello world-test");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"test".to_string()));
    }

    #[test]
    fn test_all_tokens_match_basic() {
        assert!(all_tokens_match("file system", "filesystem-access"));
    }

    #[test]
    fn test_is_subsequence() {
        assert!(is_subsequence("fss", "filesystem"));
        assert!(!is_subsequence("xyz", "filesystem"));
    }

    #[test]
    fn test_unicode_no_panic() {
        let _score = combined_similarity("café", "cafe");
        let _jw = jaro_winkler("naïve", "naive");
        let _lev = levenshtein("résumé", "resume");
        // Just verify no panics with unicode
    }
}
