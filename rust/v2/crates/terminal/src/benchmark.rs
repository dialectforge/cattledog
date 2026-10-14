// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Lu Quality Benchmark (LQB) — binary `lu_quality_bench`.
//!
//! Measures Lu's knowledge recall and narration quality across 20 questions
//! spanning 5 semantic categories, using a 10-passage corpus.
//!
//! ## Scoring (per question, 0.0–1.0 each dimension)
//!
//! | Dimension        | Measures |
//! |-----------------|---------|
//! | Relevance        | Response contains the topic's key vocabulary |
//! | Completeness     | Response contains words indicative of the expected KU type |
//! | Coherence        | Response is ≥ 10 words and contains structural connectors |
//! | No-hallucination | Response is grounded (non-empty; not a generic fallback) |
//!
//! Target: mean score across all 20 questions ≥ 0.6.

use brain::Brain;
use graph::node::Confidence;

// ── BenchmarkQuestion ─────────────────────────────────────────────────────────

#[derive(Debug)]
struct BenchmarkQuestion {
    category: &'static str,
    question: &'static str,
    /// Key words that must appear in a relevant response.
    topic_keywords: &'static [&'static str],
    /// Words that indicate the expected KU type is present.
    ku_type_words:  &'static [&'static str],
}

// ── Scoring ───────────────────────────────────────────────────────────────────

/// Fraction of `keywords` present in `response` (case-insensitive).
fn keyword_fraction(response: &str, keywords: &[&str]) -> f32 {
    if keywords.is_empty() {
        return 1.0;
    }
    let r = response.to_lowercase();
    let hits = keywords.iter().filter(|&&kw| r.contains(kw)).count();
    hits as f32 / keywords.len() as f32
}

/// Score relevance: 0–1 based on topic keyword presence.
pub fn score_relevance(response: &str, topic_keywords: &[&str]) -> f32 {
    keyword_fraction(response, topic_keywords)
}

/// Score completeness: 0–1 based on KU-type indicator words.
pub fn score_completeness(response: &str, ku_type_words: &[&str]) -> f32 {
    keyword_fraction(response, ku_type_words)
}

/// Score coherence: 0–1 based on length and sentence-structure words.
pub fn score_coherence(response: &str) -> f32 {
    let words = response.split_whitespace().count();
    if words < 5 {
        return 0.0;
    }
    let length_score = (words as f32 / 30.0).min(1.0);
    // Structural connectors suggest well-formed prose.
    const CONNECTORS: &[&str] = &[
        "is", "are", "was", "were", "works", "because", "therefore",
        "which", "that", "however", "although", "also", "known", "called",
    ];
    let r = response.to_lowercase();
    let connector_count = CONNECTORS.iter().filter(|&&c| r.contains(c)).count();
    let connector_score = (connector_count as f32 / 4.0).min(1.0);
    (length_score * 0.5 + connector_score * 0.5).min(1.0)
}

/// Score no-hallucination: 1.0 if grounded, 0.0 if generic fallback.
pub fn score_no_hallucination(response: &str, topic_keywords: &[&str]) -> f32 {
    let r = response.to_lowercase();
    // Generic fallback message → likely no knowledge → score 0.
    if r.contains("don't have knowledge") || r.contains("no knowledge") || r.len() < 15 {
        return 0.0;
    }
    // If any topic keyword is present the response is grounded.
    let grounded = topic_keywords.iter().any(|&kw| r.contains(kw));
    if grounded { 1.0 } else { 0.5 }
}

/// Compute all four dimensions and return (relevance, completeness, coherence,
/// no_hallucination, mean).
pub fn score_response(
    response: &str,
    topic_keywords: &[&str],
    ku_type_words: &[&str],
) -> (f32, f32, f32, f32, f32) {
    let rel  = score_relevance(response, topic_keywords);
    let comp = score_completeness(response, ku_type_words);
    let coh  = score_coherence(response);
    let nohal = score_no_hallucination(response, topic_keywords);
    let mean  = (rel + comp + coh + nohal) / 4.0;
    (rel, comp, coh, nohal, mean)
}

// ── Benchmark corpus ──────────────────────────────────────────────────────────

fn build_corpus() -> Vec<&'static str> {
    vec![
        // 1. Machine learning
        "Machine learning is a branch of artificial intelligence that enables computers to \
learn from data without being explicitly programmed. \
Supervised learning is a type of machine learning that uses labelled examples to train models. \
Unsupervised learning discovers hidden patterns without labelled data. \
Overfitting occurs when a model learns the training data too well and fails to generalise. \
Regularisation is a technique used to prevent overfitting in machine learning models. \
Gradient descent is an optimisation algorithm used to train machine learning models.",

        // 2. Neural networks and deep learning
        "A neural network is a computational model inspired by the human brain. \
Deep learning is a machine learning technique based on deep neural networks with many layers. \
Convolutional neural networks are used in deep learning for image recognition tasks. \
Recurrent neural networks process sequential data such as text and speech. \
Backpropagation is the algorithm that trains neural networks by adjusting weights using gradients. \
Transfer learning allows deep learning models to reuse weights pre-trained on large datasets.",

        // 3. Cryptography
        "Cryptography is the practice of securing information through mathematical techniques. \
Symmetric encryption uses the same key for both encryption and decryption. \
AES is a widely used symmetric encryption algorithm for securing data. \
Public key cryptography uses a key pair consisting of a public key and a private key. \
RSA is a popular public key cryptography algorithm based on integer factorisation. \
Hashing converts data into a fixed-size digest that cannot be reversed. \
Digital signatures use private keys to authenticate the origin of data.",

        // 4. Climate change
        "Climate change is the long-term shift in global temperatures and weather patterns. \
Greenhouse gases are the primary driver of climate change. \
Carbon dioxide emissions from burning fossil fuels cause global warming. \
Global warming causes polar ice caps to melt and sea levels to rise. \
Rising sea levels threaten coastal cities and low-lying island nations. \
The greenhouse effect traps heat in the atmosphere causing temperature rise.",

        // 5. Evolution and natural selection
        "Evolution is the change in inherited characteristics across successive generations. \
Natural selection is the mechanism by which organisms with beneficial traits survive and reproduce. \
Mutations are random changes in DNA that can introduce new traits into a population. \
Adaptation is the process by which organisms become better suited to their environment. \
Speciation occurs when populations become reproductively isolated and diverge into new species. \
Natural selection causes the frequency of beneficial traits to increase over generations.",

        // 6. Photosynthesis
        "Photosynthesis is the process by which plants convert light energy into chemical energy. \
Chlorophyll is the pigment that absorbs light energy during photosynthesis. \
Photosynthesis produces glucose and oxygen from carbon dioxide and water. \
The light-dependent reactions of photosynthesis occur in the thylakoids of the chloroplast. \
The Calvin cycle is the light-independent stage that fixes carbon dioxide into sugar. \
Plants use photosynthesis to produce the glucose needed for growth and energy.",

        // 7. Quantum computing
        "A quantum computer is a device that uses quantum mechanical phenomena to perform computation. \
Qubits are the basic unit of quantum information and differ from classical bits by existing in superposition. \
Quantum entanglement allows qubits to be correlated in ways impossible for classical bits. \
Quantum algorithms can solve certain problems exponentially faster than classical computers. \
Shor's algorithm is a quantum algorithm that can factor large numbers efficiently. \
Quantum computing differs from classical computing by using superposition and entanglement.",

        // 8. DNA and genetics
        "DNA is the molecule that carries genetic information in living organisms. \
Genes are segments of DNA that encode instructions for making proteins. \
The double helix structure of DNA consists of two complementary strands. \
Transcription is the process of copying DNA into messenger RNA. \
Translation converts messenger RNA into a protein sequence at the ribosome. \
DNA differs from RNA in that DNA is double-stranded and contains thymine instead of uracil.",

        // 9. Thermodynamics
        "Thermodynamics is the study of heat and its relationship to other forms of energy. \
The first law of thermodynamics states that energy cannot be created or destroyed. \
The second law of thermodynamics states that entropy in an isolated system always increases. \
Temperature is a measure of the average kinetic energy of particles in a substance. \
Heat transfer occurs through conduction, convection, and radiation. \
Entropy always increases because there are more disordered states than ordered states.",

        // 10. Algorithms and complexity
        "An algorithm is a step-by-step procedure for solving a computational problem. \
Sorting algorithms arrange elements in a specific order such as ascending or descending. \
Binary search is an efficient algorithm for finding elements in a sorted array. \
Time complexity measures how the runtime of an algorithm scales with input size. \
Big O notation is used to express the upper bound of an algorithm's time complexity. \
Binary search is more efficient than linear search because it halves the search space each step.",
    ]
}

// ── Benchmark questions ───────────────────────────────────────────────────────

fn build_questions() -> Vec<BenchmarkQuestion> {
    vec![
        // ── Factual Recall ──────────────────────────────────────────────────
        BenchmarkQuestion {
            category: "Factual Recall",
            question: "What is machine learning?",
            topic_keywords: &["machine", "learning", "data", "algorithm"],
            ku_type_words:  &["is", "branch", "enables", "technique", "defined"],
        },
        BenchmarkQuestion {
            category: "Factual Recall",
            question: "What is photosynthesis?",
            topic_keywords: &["photosynthesis", "light", "plant", "glucose"],
            ku_type_words:  &["is", "process", "converts", "produces", "defined"],
        },
        BenchmarkQuestion {
            category: "Factual Recall",
            question: "What is a quantum computer?",
            topic_keywords: &["quantum", "computer", "qubit", "computation"],
            ku_type_words:  &["is", "device", "uses", "phenomenon", "defined"],
        },
        BenchmarkQuestion {
            category: "Factual Recall",
            question: "What is DNA?",
            topic_keywords: &["dna", "molecule", "genetic", "protein"],
            ku_type_words:  &["is", "carries", "information", "structure"],
        },

        // ── Mechanism Understanding ─────────────────────────────────────────
        BenchmarkQuestion {
            category: "Mechanism Understanding",
            question: "How does natural selection work?",
            topic_keywords: &["natural", "selection", "traits", "survive"],
            ku_type_words:  &["works", "mechanism", "causes", "process", "organisms"],
        },
        BenchmarkQuestion {
            category: "Mechanism Understanding",
            question: "How does deep learning work?",
            topic_keywords: &["deep", "learning", "neural", "layers"],
            ku_type_words:  &["works", "based", "trains", "process", "using"],
        },
        BenchmarkQuestion {
            category: "Mechanism Understanding",
            question: "How does photosynthesis work?",
            topic_keywords: &["photosynthesis", "light", "carbon", "glucose"],
            ku_type_words:  &["works", "process", "converts", "produces", "reactions"],
        },
        BenchmarkQuestion {
            category: "Mechanism Understanding",
            question: "How does backpropagation work?",
            topic_keywords: &["backpropagation", "neural", "weights", "gradient"],
            ku_type_words:  &["works", "trains", "algorithm", "adjusting", "using"],
        },

        // ── Comparison ─────────────────────────────────────────────────────
        BenchmarkQuestion {
            category: "Comparison",
            question: "What is the difference between machine learning and deep learning?",
            topic_keywords: &["machine", "deep", "learning", "neural"],
            ku_type_words:  &["whereas", "unlike", "differs", "while", "compared"],
        },
        BenchmarkQuestion {
            category: "Comparison",
            question: "How does quantum computing differ from classical computing?",
            topic_keywords: &["quantum", "classical", "computing", "qubit"],
            ku_type_words:  &["differs", "unlike", "whereas", "compared", "while"],
        },
        BenchmarkQuestion {
            category: "Comparison",
            question: "What is the difference between symmetric and public key cryptography?",
            topic_keywords: &["symmetric", "public", "key", "encryption"],
            ku_type_words:  &["differs", "whereas", "unlike", "compared", "while"],
        },
        BenchmarkQuestion {
            category: "Comparison",
            question: "How does supervised learning differ from unsupervised learning?",
            topic_keywords: &["supervised", "unsupervised", "learning", "labelled"],
            ku_type_words:  &["differs", "whereas", "unlike", "labelled", "without"],
        },

        // ── Causal Reasoning ────────────────────────────────────────────────
        BenchmarkQuestion {
            category: "Causal Reasoning",
            question: "Why does climate change cause rising sea levels?",
            topic_keywords: &["climate", "sea", "ice", "warming"],
            ku_type_words:  &["causes", "because", "leads", "results", "therefore"],
        },
        BenchmarkQuestion {
            category: "Causal Reasoning",
            question: "Why do mutations lead to evolutionary change?",
            topic_keywords: &["mutations", "evolution", "traits", "natural"],
            ku_type_words:  &["causes", "because", "leads", "introduce", "frequency"],
        },
        BenchmarkQuestion {
            category: "Causal Reasoning",
            question: "What causes overfitting in machine learning?",
            topic_keywords: &["overfitting", "training", "model", "learning"],
            ku_type_words:  &["causes", "because", "occurs", "when", "too"],
        },
        BenchmarkQuestion {
            category: "Causal Reasoning",
            question: "Why does entropy always increase?",
            topic_keywords: &["entropy", "thermodynamics", "disorder", "states"],
            ku_type_words:  &["because", "more", "disordered", "second", "always"],
        },

        // ── Multi-part Synthesis ────────────────────────────────────────────
        BenchmarkQuestion {
            category: "Multi-part Synthesis",
            question: "Explain machine learning, its types, and how it works",
            topic_keywords: &["machine", "learning", "supervised", "data"],
            ku_type_words:  &["is", "type", "works", "trains", "algorithm"],
        },
        BenchmarkQuestion {
            category: "Multi-part Synthesis",
            question: "Explain photosynthesis, its stages, and what it produces",
            topic_keywords: &["photosynthesis", "light", "glucose", "chlorophyll"],
            ku_type_words:  &["process", "stage", "produces", "converts", "reactions"],
        },
        BenchmarkQuestion {
            category: "Multi-part Synthesis",
            question: "Explain DNA, its structure, and how it encodes information",
            topic_keywords: &["dna", "structure", "genetic", "protein"],
            ku_type_words:  &["double", "helix", "encodes", "genes", "strands"],
        },
        BenchmarkQuestion {
            category: "Multi-part Synthesis",
            question: "Explain climate change, its causes, and effects",
            topic_keywords: &["climate", "change", "greenhouse", "warming"],
            ku_type_words:  &["causes", "emissions", "sea", "effects", "fossil"],
        },
    ]
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║          Lu Quality Benchmark (LQB) — v2 Phase 6                ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    // ── Build brain and ingest corpus ─────────────────────────────────────────

    let mut brain = Brain::new();

    println!("Ingesting benchmark corpus (10 passages)…");
    for passage in build_corpus() {
        brain.learn_text(passage, Confidence::Terminal);
    }

    let stats = brain.stats();
    println!(
        "Corpus ready: {} nodes, {} bonds, {} index words",
        stats.total_nodes,
        stats.total_bonds,
        brain.network.keyword_index_size(),
    );
    println!();

    // ── Run benchmark ─────────────────────────────────────────────────────────

    let questions = build_questions();
    let mut category_totals: std::collections::HashMap<&str, Vec<f32>> =
        std::collections::HashMap::new();
    let mut all_scores: Vec<f32> = Vec::new();

    println!("{:<26} {:<5} {:<5} {:<5} {:<5} {:<5}",
        "Question", "Rel", "Comp", "Coh", "NoH", "Mean");
    println!("{}", "─".repeat(55));

    for q in &questions {
        let response = brain.respond(q.question);
        let (rel, comp, coh, nohal, mean) =
            score_response(&response, q.topic_keywords, q.ku_type_words);

        // Truncate question for display
        let q_short = if q.question.len() > 25 {
            format!("{}…", &q.question[..24])
        } else {
            q.question.to_string()
        };

        println!(
            "{:<26} {:.2}  {:.2}  {:.2}  {:.2}  {:.2}",
            q_short, rel, comp, coh, nohal, mean
        );

        category_totals
            .entry(q.category)
            .or_default()
            .push(mean);
        all_scores.push(mean);
    }

    println!("{}", "─".repeat(55));
    println!();

    // ── Per-category summary ──────────────────────────────────────────────────

    println!("Per-category averages:");
    let mut categories: Vec<&str> = category_totals.keys().copied().collect();
    categories.sort_unstable();

    for cat in &categories {
        let scores = &category_totals[cat];
        let avg: f32 = scores.iter().sum::<f32>() / scores.len() as f32;
        let mark = if avg >= 0.6 { "✓" } else { "✗" };
        println!("  {} {:<26} {:.3}", mark, cat, avg);
    }

    // ── Overall ───────────────────────────────────────────────────────────────

    let overall: f32 = all_scores.iter().sum::<f32>() / all_scores.len() as f32;
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  Overall LQB score: {:.3} / 1.000  (target ≥ 0.600)              ║", overall);
    if overall >= 0.6 {
        println!("║  RESULT: PASS ✓                                                  ║");
    } else {
        println!("║  RESULT: BELOW TARGET ✗  — review failing categories above       ║");
    }
    println!("╚══════════════════════════════════════════════════════════════════╝");

    if overall < 0.6 {
        // Print first 5 low-scoring question details for debugging.
        println!();
        println!("Low-score diagnostics (questions scoring < 0.6):");
        for q in &questions {
            let response = brain.respond(q.question);
            let (_, _, _, _, mean) =
                score_response(&response, q.topic_keywords, q.ku_type_words);
            if mean < 0.6 {
                println!("  Q: {}", q.question);
                println!("  A: {}…", response.chars().take(120).collect::<String>());
                println!();
            }
        }
        std::process::exit(1);
    }
}
