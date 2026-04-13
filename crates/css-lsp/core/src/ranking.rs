pub fn similarity_score(query: &str, candidate: &str) -> i32 {
    let query = query.trim().to_ascii_lowercase();
    let candidate = candidate.trim().to_ascii_lowercase();

    if query.is_empty() {
        return 0;
    }
    if query == candidate {
        return 10_000;
    }

    let mut score = 0;

    if candidate.starts_with(&query) {
        score += 5_000;
    }
    if candidate.split(['-', '_', ' ', '.']).any(|segment| segment.starts_with(&query)) {
        score += 6_000;
    }
    if candidate.contains(&query) {
        score += 2_000;
    }

    score += subsequence_score(&query, &candidate);
    score -= (candidate.len() as i32 - query.len() as i32).abs();
    score
}

fn subsequence_score(query: &str, candidate: &str) -> i32 {
    let mut score = 0;
    let mut candidate_index = 0usize;
    let candidate_bytes = candidate.as_bytes();

    for query_byte in query.bytes() {
        let mut matched = false;
        while candidate_index < candidate_bytes.len() {
            if candidate_bytes[candidate_index] == query_byte {
                score += 10;
                matched = true;
                candidate_index += 1;
                break;
            }
            candidate_index += 1;
        }
        if !matched {
            return -1_000;
        }
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_closer_matches() {
        assert!(similarity_score("stack", "stack-group") > similarity_score("stack", "shell"));
        assert!(similarity_score("shell", "shell") > similarity_score("shell", "stack-shell"));
    }
}
