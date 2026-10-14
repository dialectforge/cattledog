// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC

/**
 * Tauri invoke wrappers — typed calls to the Rust backend.
 * Uses the native Tauri IPC bridge (no HTTP, no axios).
 */
import { invoke } from "@tauri-apps/api/core";

// ── DTOs (must match src-tauri/src/lib.rs) ────────────────────────────────────

export interface ChatResponse {
  text: string;
  timing_ms: number | null;
  nodes_activated: number | null;
}

export interface ThinkResult {
  answer: string;
  timing_ms: number;
}

export interface BrainStatus {
  nodes_total: number;
  nodes_active: number;
  bonds_total: number;
  bonds_active: number;
  episodes: number;
  scars: number;
  autonomy_tier: string;
  pending_proposals: number;
  recall_hit_rate: number;
  avg_response_quality: number;
  consolidation_effectiveness: number;
  total_queries: number;
  warnings: string[];
}

export interface FlagItem {
  name: string;
  enabled: boolean;
}

export interface GraphNode {
  id: string;
  label: string;
  confidence: string;
  ku_type: string;
  activation_count: number;
}

export interface GraphBond {
  id: string;
  source: string;
  target: string;
  bond_type: string;
  weight: number;
}

export interface GraphData {
  nodes: GraphNode[];
  bonds: GraphBond[];
  total_nodes: number;
  total_bonds: number;
}

export interface LearnResult {
  count: number;
  seeds: string[];
}

export interface ConsolidateResult {
  episodes_processed: number;
  bonds_strengthened: number;
}

export interface ProposalItem {
  id: string;
  kind: string;
  summary: string;
  created_at: string;
}

export interface ObserveData {
  active_nodes: number;
  active_bonds: number;
  episodes_total: number;
  pending_consolidation: number;
  curiosity_queue: number;
  wonder_items: string[];
}

export interface WebSearchResult {
  query: string;
  pages_visited: number;
  facts_extracted: number;
  sources: string[];
  summary: string;
  elapsed_secs: number;
}

export interface CuriosityResult {
  new_items: number;
  pending: number;
  top_questions: string[];
}

export interface DaemonToggleResult {
  running: boolean;
  message: string;
}

// ── API calls ─────────────────────────────────────────────────────────────────

export const api = {
  chat:    (message: string) =>
    invoke<ChatResponse>("cmd_chat", { message }),

  think:   (topic: string) =>
    invoke<ThinkResult>("cmd_think", { topic }),

  learn:   (content: string, confidence = "Terminal") =>
    invoke<LearnResult>("cmd_learn", { content, confidence }),

  learnFile: (path: string) =>
    invoke<LearnResult>("cmd_learn_file", { path }),

  consolidate: () =>
    invoke<ConsolidateResult>("cmd_consolidate"),

  graphData: (limit?: number) =>
    invoke<GraphData>("cmd_graph_data", { limit: limit ?? null }),

  proposalsList: () =>
    invoke<ProposalItem[]>("cmd_proposals_list"),

  proposalApprove: (proposalId: string) =>
    invoke<string>("cmd_proposal_approve", { proposalId }),

  proposalReject: (proposalId: string) =>
    invoke<string>("cmd_proposal_reject", { proposalId }),

  status:  () =>
    invoke<BrainStatus>("cmd_status"),

  observe: () =>
    invoke<ObserveData>("cmd_observe"),

  flagsList: () =>
    invoke<FlagItem[]>("cmd_flags_list"),

  flagSet: (name: string, value: boolean) =>
    invoke<void>("cmd_flag_set", { name, value }),

  configure: (autonomyTier?: string, storagePath?: string) =>
    invoke<string>("cmd_configure", {
      autonomyTier: autonomyTier ?? null,
      storagePath:  storagePath  ?? null,
    }),

  save: () =>
    invoke<string>("cmd_save"),

  load: (path: string) =>
    invoke<string>("cmd_load", { path }),

  revoke: (seedHex: string, reason: string) =>
    invoke<string>("cmd_revoke", { seedHex, reason }),

  webSearch: (query: string) =>
    invoke<WebSearchResult>("cmd_web_search", { query }),

  curiosityTrigger: () =>
    invoke<CuriosityResult>("cmd_curiosity_trigger"),

  toggleDaemon: () =>
    invoke<DaemonToggleResult>("cmd_toggle_daemon"),
};
