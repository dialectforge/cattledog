// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
// ─── Tokenizer ──────────────────────────────────────────────────
//
// Stage 1 of the DF pipeline: Tokenizer -> Cascade -> Intent -> Assembler
//
// The tokenizer breaks raw input into weighted tokens. Unlike LLM
// tokenizers (BPE/WordPiece), this one uses the BRAIN ITSELF to
// recognize multi-word concepts.
//
// "Big Bang" in the graph -> one token, not two.
// "physics" -> content token, weight 1.0.
//
// The vocabulary IS the tokenizer. The graph teaches itself what
// a "token" is. This is uniquely DF -- no other system does this.
//
// ────────────────────────────────────────────────────────────────

use std::collections::HashSet;

/// A single token produced by the tokenizer.
/// Each token carries a weight that flows into the activation cascade.
#[derive(Clone, Debug)]
pub struct Token {
    /// The text of this token (lowercase, trimmed).
    /// Multi-word tokens are space-joined: "big bang"
    pub text: String,

    /// Weight for the activation cascade.
    /// 0.0 = single-char filler
    /// 1.0 = normal content word
    /// 1.5 = multi-word concept (recognized from graph)
    /// The cascade multiplies activation by this weight.
    pub weight: f32,

    /// How many original words this token spans.
    /// "big bang" -> span = 2, "physics" -> span = 1
    pub span: usize,

    /// Whether this token is a known multi-word concept
    pub is_compound: bool,
}

/// The tokenizer output -- a sequence of weighted tokens
/// plus metadata about what was recognized.
#[derive(Clone, Debug)]
pub struct TokenizedQuery {
    /// The weighted tokens, in order
    pub tokens: Vec<Token>,

    /// Content tokens only (weight > 0), for quick access
    pub content_tokens: Vec<Token>,

    /// The original query text
    pub original: String,

    /// Multi-word concepts found (for debugging/display)
    pub compounds_found: Vec<String>,
}

/// Known multi-word phrases extracted from the brain's graph.
/// Built from the graph's node labels. This IS the "learned
/// vocabulary" -- concepts the brain knows become tokenization
/// units. As the brain learns more, it recognizes more compounds.
pub struct PhraseIndex {
    /// 2-word phrases: "big bang", "black hole", "machine learning"
    bigrams: HashSet<String>,
    /// 3-word phrases: "big bang theory", "central nervous system"
    trigrams: HashSet<String>,
}


impl PhraseIndex {
    /// Build phrase index from node labels in the graph.
    ///
    /// Scans every node label and extracts 2-word and 3-word
    /// sequences that appear at the START of labels. Starting
    /// position matters -- "Big Bang nucleosynthesis" gives us
    /// "big bang" as a concept, but we don't extract "bang
    /// nucleosynthesis" because that's not a meaningful unit.
    ///
    /// Also extracts short labels (2-3 words) as complete concepts.
    pub fn build<'a>(labels: impl Iterator<Item = &'a str>) -> Self {
        let mut bigrams = HashSet::new();
        let mut trigrams = HashSet::new();
        let mut bigram_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut trigram_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for label in labels {
            let lower = label.to_lowercase();
            let words: Vec<&str> = lower.split_whitespace().collect();

            // Extract leading bigrams (first 2 words of label)
            if words.len() >= 2 {
                let bg = format!("{} {}", words[0], words[1]);
                *bigram_counts.entry(bg).or_insert(0) += 1;
            }

            // Extract leading trigrams (first 3 words)
            if words.len() >= 3 {
                let tg = format!("{} {} {}", words[0], words[1], words[2]);
                *trigram_counts.entry(tg).or_insert(0) += 1;
            }

            // Short labels (2-3 words) are complete concepts
            if words.len() == 2 {
                let bg = format!("{} {}", words[0], words[1]);
                *bigram_counts.entry(bg).or_insert(0) += 1;
            }
            if words.len() == 3 {
                let tg = format!("{} {} {}", words[0], words[1], words[2]);
                *trigram_counts.entry(tg).or_insert(0) += 1;
            }
        }

        // Only keep phrases that appear in 2+ node labels.
        // A phrase appearing once might be coincidence;
        // appearing multiple times means the brain knows it as a concept.
        for (phrase, count) in bigram_counts {
            if count >= 2 {
                bigrams.insert(phrase);
            }
        }
        for (phrase, count) in trigram_counts {
            if count >= 2 {
                trigrams.insert(phrase);
            }
        }

        PhraseIndex { bigrams, trigrams }
    }

    /// Check if a 2-word sequence is a known concept
    pub fn is_bigram(&self, w1: &str, w2: &str) -> bool {
        let key = format!("{} {}", w1.to_lowercase(), w2.to_lowercase());
        self.bigrams.contains(&key)
    }

    /// Check if a 3-word sequence is a known concept
    pub fn is_trigram(&self, w1: &str, w2: &str, w3: &str) -> bool {
        let key = format!("{} {} {}",
            w1.to_lowercase(), w2.to_lowercase(), w3.to_lowercase());
        self.trigrams.contains(&key)
    }

    /// Stats for debugging
    pub fn stats(&self) -> (usize, usize) {
        (self.bigrams.len(), self.trigrams.len())
    }
}


/// Tokenize a query using the brain's phrase index.
///
/// This is the main entry point. Given raw text and a phrase index
/// built from the graph, produces weighted tokens.
///
/// Algorithm:
/// 1. Split into words
/// 2. Greedy left-to-right scan: try trigram first, then bigram,
///    then single word
/// 3. Assign weights: compounds get 1.5, content words get 1.0,
///    single-char words get 0.0
pub fn tokenize(query: &str, phrases: &PhraseIndex) -> TokenizedQuery {
    let original = query.to_string();
    let clean = query.trim().to_lowercase();
    let clean = clean.trim_end_matches(&['?', '!', '.'][..]).trim();

    let words: Vec<&str> = clean.split_whitespace().collect();
    let mut tokens: Vec<Token> = Vec::new();
    let mut compounds_found: Vec<String> = Vec::new();

    let mut i = 0;
    while i < words.len() {
        // Try trigram first (greedy longest match)
        if i + 2 < words.len() {
            if phrases.is_trigram(words[i], words[i+1], words[i+2]) {
                let text = format!("{} {} {}", words[i], words[i+1], words[i+2]);
                compounds_found.push(text.clone());
                tokens.push(Token {
                    text,
                    weight: 1.5,
                    span: 3,
                    is_compound: true,
                });
                i += 3;
                continue;
            }
        }

        // Try bigram
        if i + 1 < words.len() {
            if phrases.is_bigram(words[i], words[i+1]) {
                let text = format!("{} {}", words[i], words[i+1]);
                compounds_found.push(text.clone());
                tokens.push(Token {
                    text,
                    weight: 1.5,
                    span: 2,
                    is_compound: true,
                });
                i += 2;
                continue;
            }
        }

        // Single word
        let word = words[i];
        let weight = if word.len() < 2 {
            0.0  // single chars aren't meaningful
        } else {
            1.0
        };

        tokens.push(Token {
            text: word.to_string(),
            weight,
            span: 1,
            is_compound: false,
        });
        i += 1;
    }

    // Build content_tokens (weight > 0)
    let content_tokens: Vec<Token> = tokens.iter()
        .filter(|t| t.weight > 0.0)
        .cloned()
        .collect();

    TokenizedQuery {
        tokens,
        content_tokens,
        original,
        compounds_found,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compound_detection() {
        let mut bigrams = HashSet::new();
        bigrams.insert("big bang".to_string());
        let phrases = PhraseIndex { bigrams, trigrams: HashSet::new() };

        let result = tokenize("what is the big bang", &phrases);
        assert!(result.compounds_found.contains(&"big bang".to_string()));
        // "big bang" should be one token with weight 1.5
        let bb = result.content_tokens.iter()
            .find(|t| t.text == "big bang")
            .expect("big bang token not found");
        assert_eq!(bb.weight, 1.5);
        assert_eq!(bb.span, 2);
        assert!(bb.is_compound);
    }

    #[test]
    fn test_single_word_query() {
        let phrases = PhraseIndex {
            bigrams: HashSet::new(),
            trigrams: HashSet::new(),
        };
        let result = tokenize("angry", &phrases);
        assert_eq!(result.content_tokens.len(), 1);
        assert_eq!(result.content_tokens[0].text, "angry");
        assert_eq!(result.content_tokens[0].weight, 1.0);
    }
}
