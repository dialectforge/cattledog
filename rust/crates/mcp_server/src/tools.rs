// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # MCP Tool Definitions and Handlers
//!
//! Every brain operation exposed as an MCP tool.
//! Claude Desktop calls these to train the brain at scale.

use brain::Brain;
use df_core::Seed;
use serde_json::{json, Value};
use brain::coordinator;
use crate::protocol;

/// Return the tool list for tools/list.
pub fn handle_list(id: &Value) -> String {
    let tools = json!({
        "tools": [
            {
                "name": "brain_learn",
                "description": "Teach the brain a new fact. Creates a knowledge node with highest trust (Terminal confidence). Returns the seed of the new node. Use for direct teaching.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "The knowledge to teach"
                        }
                    },
                    "required": ["content"]
                },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_associate",
                "description": "Create an association bond between two knowledge nodes by their seed hex strings. Bonds enable spreading activation traversal between related concepts. Weight controls bond strength (default 1.0). Use weight 3.0 for vocabulary definitions.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "seed_a": {
                            "type": "string",
                            "description": "Full hex seed of first node"
                        },
                        "seed_b": {
                            "type": "string",
                            "description": "Full hex seed of second node"
                        },
                        "description": {
                            "type": "string",
                            "description": "What this association means"
                        },
                        "weight": {
                            "type": "number",
                            "description": "Bond strength (default 1.0). Higher = more activation flows through this bond. Use 3.0 for definitions."
                        }
                    },
                    "required": ["seed_a", "seed_b", "description"]
                }
            },
            {
                "name": "brain_revoke",
                "description": "Revoke a knowledge node or bond by seed. Leaves a permanent scar that warns when similar associations try to form. Forward traces to children. This is structural safety -- the path is severed, not hidden.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "seed": {
                            "type": "string",
                            "description": "Full hex seed to revoke"
                        },
                        "reason": {
                            "type": "string",
                            "description": "Why this is being revoked"
                        }
                    },
                    "required": ["seed", "reason"]
                },
                "annotations": {
                    "destructiveHint": true
                }
            },
            {
                "name": "brain_propose",
                "description": "Propose learning external content. Returns a proposal with similarity analysis, scar conflicts, and recommendation. Must be approved or rejected before it becomes knowledge.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "Content to propose learning"
                        },
                        "source": {
                            "type": "string",
                            "description": "Where this content came from"
                        }
                    },
                    "required": ["content", "source"]
                }
            },
            {
                "name": "brain_approve",
                "description": "Approve a pending learning proposal by its ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "proposal_id": {
                            "type": "string",
                            "description": "The proposal ID to approve"
                        }
                    },
                    "required": ["proposal_id"]
                }
            },
            {
                "name": "brain_reject",
                "description": "Reject a pending learning proposal by its ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "proposal_id": {
                            "type": "string",
                            "description": "The proposal ID to reject"
                        }
                    },
                    "required": ["proposal_id"]
                }
            },
            {
                "name": "brain_stats",
                "description": "Get brain statistics: active nodes, revoked nodes, bonds, scars, bond state.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                },
                "annotations": {
                    "readOnlyHint": true
                }
            },
            {
                "name": "brain_scars",
                "description": "List all revocation scars. Scars are permanent warnings left when knowledge is revoked.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                },
                "annotations": {
                    "readOnlyHint": true
                }
            },
            {
                "name": "brain_seed_report",
                "description": "Get the seed report from the last recall -- which seeds contributed what percentage.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                },
                "annotations": {
                    "readOnlyHint": true
                }
            },
            {
                "name": "brain_batch_train",
                "description": "Teach the brain multiple facts at once. Accepts an array of knowledge items. Each item has 'content' (required) and optional 'associations' (array of content strings to auto-bond with). Returns array of seeds created. This is the primary training tool -- use it to feed structured knowledge efficiently.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "items": {
                            "type": "array",
                            "description": "Array of knowledge items to teach",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "content": {
                                        "type": "string",
                                        "description": "The knowledge to teach"
                                    },
                                    "associations": {
                                        "type": "array",
                                        "items": { "type": "string" },
                                        "description": "Other content strings this should be associated with"
                                    }
                                },
                                "required": ["content"]
                            }
                        }
                    },
                    "required": ["items"]
                }
            },
            {
                "name": "brain_session_report",
                "description": "Generate a session report: preview what a batch of knowledge items would do BEFORE committing. Shows new atoms, associations, scar conflicts, duplicates. Nothing is changed. Use this to review before batch_train.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "items": {
                            "type": "array",
                            "description": "Array of knowledge items to preview",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "content": {
                                        "type": "string",
                                        "description": "The knowledge to preview"
                                    },
                                    "associations": {
                                        "type": "array",
                                        "items": { "type": "string" },
                                        "description": "Other content strings this would be associated with"
                                    }
                                },
                                "required": ["content"]
                            }
                        }
                    },
                    "required": ["items"]
                }
            },
            {
                "name": "brain_self_reflect",
                "description": "Trigger self-reflection after the last recall. The brain examines co-activated nodes and strengthens bonds between them. Creates novel associations between co-activated nodes that aren't yet bonded. This is use-based plasticity -- the brain rewires itself through use.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "brain_decay",
                "description": "Apply organic decay to all bonds. Bonds that haven't been used weaken. Dead bonds (weight=0) are deactivated. This is organic forgetting. Amount controls decay rate (default 0.01).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "amount": {
                            "type": "number",
                            "description": "Decay amount per bond (default 0.01)",
                            "default": 0.01
                        }
                    }
                }
            },
            {
                "name": "brain_prune",
                "description": "Remove dead bonds (weight=0 or inactive) from the network permanently. Returns count of bonds pruned.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "brain_self_learn",
                "description": "The brain generates its own knowledge atom -- a self-derived insight. Takes content and optional parent seed hexes for lineage tracking. Checks scar conflicts and deduplication. Returns the new seed or null if blocked.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "The self-generated insight"
                        },
                        "parent_seeds": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Hex seeds of parent nodes this insight derives from"
                        }
                    },
                    "required": ["content"]
                }
            },
            {
                "name": "brain_strengthen_bond",
                "description": "Strengthen a specific bond between two nodes. The brain consciously reinforces a connection.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "seed_a": { "type": "string", "description": "First node seed hex" },
                        "seed_b": { "type": "string", "description": "Second node seed hex" },
                        "amount": { "type": "number", "description": "Strength increase (default 0.1)", "default": 0.1 }
                    },
                    "required": ["seed_a", "seed_b"]
                }
            },
            {
                "name": "brain_weaken_bond",
                "description": "Weaken a specific bond between two nodes. The brain consciously weakens a connection.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "seed_a": { "type": "string", "description": "First node seed hex" },
                        "seed_b": { "type": "string", "description": "Second node seed hex" },
                        "amount": { "type": "number", "description": "Weakness amount (default 0.1)", "default": 0.1 }
                    },
                    "required": ["seed_a", "seed_b"]
                }
            },
            {
                "name": "brain_respond",
                "description": "Ask the brain a question and get a composed response. The brain activates relevant knowledge atoms through spreading activation, then stitches them into a coherent answer. No LLM -- the response emerges from the pattern of activated atoms. Also triggers self-reflection (use-based plasticity). Returns the composed text, contributing atoms, coherence score, and scar warnings.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The question or prompt to respond to"
                        },
                        "depth": {
                            "type": "integer",
                            "description": "Traversal depth -- how many hops to explore (default 3)",
                            "default": 3
                        },
                        "top_k": {
                            "type": "integer",
                            "description": "Max atoms to consider (default 10)",
                            "default": 10
                        }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "brain_ingest_text",
                "description": "Ingest raw text into the brain. Splits into sentences, checks each against scars and existing knowledge, returns proposals for review. Nothing is auto-learned -- every sentence becomes a proposal that must be approved. 'The AI observes everything, it obeys only the terminal.'",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "The raw text to ingest"
                        },
                        "source": {
                            "type": "string",
                            "description": "Where this text came from (e.g. 'wikipedia', 'textbook', 'user_notes')"
                        }
                    },
                    "required": ["text", "source"]
                }
            },
            {
                "name": "brain_ingest_file",
                "description": "Ingest a file from disk into the brain. Detects file type by extension: images (.png, .jpg, .jpeg, .gif, .bmp, .webp) and audio (.wav, .mp3, .ogg, .flac) produce a single READY_IMAGE or READY_AUDIO proposal; text (.txt, .md) and PDF (.pdf) split into per-sentence proposals. All proposals require approval via brain_approve_media (media) or brain_approve_ingest (text). Nothing is auto-learned.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute path to the file on disk"
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "brain_approve_ingest",
                "description": "Bulk approve READY proposals from an ingestion. Skips any proposals with scar conflicts (those need individual review). Returns seeds of all newly learned nodes.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "proposal_ids": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Array of proposal IDs to approve"
                        }
                    },
                    "required": ["proposal_ids"]
                }
            },
            {
                "name": "brain_save",
                "description": "Save brain state to disk.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "brain_reason",
                "description": "Perform multi-hop reasoning on a query. Goes beyond simple recall by analyzing traversal paths to find reasoning chains -- logical connections between knowledge atoms. Returns direct answer, reasoning chains with confidence scores, and a synthesized conclusion.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The question to reason about"
                        },
                        "depth": {                            "type": "integer",
                            "description": "Reasoning depth in hops (default 3)",
                            "default": 3
                        }
                    },
                    "required": ["query"]
                },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_ask",
                "description": "Ask Lu a question and get a conversational response. Uses the narration engine to convert graph traversal into natural speech. This is Lu's voice -- no LLM, just activated knowledge stitched together with bond-label connectors.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The question to ask Lu"
                        },
                        "depth": {
                            "type": "integer",
                            "description": "How many hops to traverse (default 3)",
                            "default": 3
                        },
                        "top_k": {
                            "type": "integer",
                            "description": "Max atoms to consider (default 10)",
                            "default": 10
                        }
                    },
                    "required": ["query"]
                },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_reindex",
                "description": "Scan the entire graph and create word-level bonds between nodes that share meaningful words. Uses an inverted index for efficiency -- stays fast even at 100k+ nodes. Automatically skips stop words and high-frequency words to avoid turning the graph into soup. Returns stats on bonds created/strengthened.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "max_freq_pct": {
                            "type": "number",
                            "description": "Skip words appearing in more than this % of nodes (default 5.0). Lower = stricter filtering.",
                            "default": 5.0
                        },
                        "max_per_word": {
                            "type": "integer",
                            "description": "Max bond pairs per word bucket (default 50). Prevents quadratic blowup on medium-frequency words.",
                            "default": 50
                        }
                    }
                },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false,
                    "idempotentHint": false
                }
            },
            {
                "name": "brain_think",
                "description": "Start a deliberate thinking session. Lu maintains a goal, decomposes it into sub-goals, and runs a Gather -> Plan -> Act -> Observe -> Loop cycle. All proposed changes require human approval. Only one thinking task can be active at a time. Returns initial state with first proposals.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "goal": {
                            "type": "string",
                            "description": "The goal or question to think about"
                        },
                        "max_iterations": {
                            "type": "integer",
                            "description": "Maximum thinking iterations (default 5, max 20)",
                            "default": 5
                        },
                        "coherence_threshold": {
                            "type": "number",
                            "description": "Coherence score at which thinking is done (default 0.6)",
                            "default": 0.6
                        },
                        "dry_run": {
                            "type": "boolean",
                            "description": "If true, no real nodes/bonds are created -- preview only",
                            "default": false
                        },
                        "auto_approve_safe": {
                            "type": "boolean",
                            "description": "If true, non-destructive proposals auto-approve (sub-goals, weak hypotheses)",
                            "default": false
                        }
                    },
                    "required": ["goal"]
                }
            },
            {
                "name": "brain_think_step",
                "description": "Advance one iteration of the thinking loop. The state machine determines what happens: Gathering -> Planning -> AwaitingApproval -> Acting -> Observing -> Loop. Returns current state with any new proposals that need approval.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "brain_think_approve",
                "description": "Approve a specific thinking proposal by its ID. Proposals include sub-goal creation, hypothesis bonds, insight learning, bond promotion, or contradiction revocation.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "proposal_id": {
                            "type": "string",
                            "description": "The proposal ID to approve"
                        }
                    },
                    "required": ["proposal_id"]
                }
            },
            {
                "name": "brain_think_reject",
                "description": "Reject a specific thinking proposal by its ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "proposal_id": {
                            "type": "string",
                            "description": "The proposal ID to reject"
                        }
                    },
                    "required": ["proposal_id"]
                }
            },
            {
                "name": "brain_think_status",
                "description": "Get the current thinking state: goal, state machine position, iteration, coherence, sub-goal count, pending proposals. Read-only -- does not advance the loop.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_think_stop",
                "description": "End the thinking session. Cleans up temporary nodes/bonds that were not promoted. Returns a summary: iterations completed, final coherence, nodes created/promoted/cleaned, observations, and a narrated conclusion.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                },
                "annotations": {
                    "destructiveHint": true
                }
            },
            {
                "name": "brain_think_simulate",
                "description": "Dry-run thinking: run a full think loop (gather→plan→act→observe) without creating real nodes/bonds. Returns what WOULD happen. Safe preview mode.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "goal": { "type": "string", "description": "The thinking goal to simulate" }
                    },
                    "required": ["goal"]
                },
                "annotations": { "readOnlyHint": true, "destructiveHint": false }
            },
            {
                "name": "brain_approve_media",
                "description": "Approve a pending media proposal (image, audio, or video) by its proposal ID. Creates a Media node in the network with the encoded pattern vector and metadata. Use after brain_ingest_file returns a READY_IMAGE, READY_AUDIO, or READY_VIDEO proposal.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "proposal_id": {
                            "type": "string",
                            "description": "The proposal ID to approve (from brain_ingest_file output)"
                        }
                    },
                    "required": ["proposal_id"]
                }
            },
            {
                "name": "brain_reject_media",
                "description": "Reject a pending media proposal (image, audio, or video) by its proposal ID. Discards the proposal without creating a node.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "proposal_id": {
                            "type": "string",
                            "description": "The proposal ID to reject"
                        }
                    },
                    "required": ["proposal_id"]
                }
            },
            {
                "name": "brain_cross_modal_search",
                "description": "Given a text query, find related Media nodes (images, audio, video) across modalities. Uses cosine similarity on 256-dim pattern vectors with a lower threshold (0.70) to account for modality offset. Returns matching media nodes with similarity scores.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Text query to search across media nodes"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Max results to return (default 10)"
                        }
                    },
                    "required": ["query"]
                },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_session_context",
                "description": "Add context text to the current Left Brain session memory. Session context informs recall but never auto-promotes to permanent knowledge. Use to record user statements, observations, or working beliefs that should influence this session's reasoning without becoming permanent facts.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "The context or belief to add to session memory"
                        },
                        "source": {
                            "type": "string",
                            "description": "Where this came from (e.g. 'user_statement', 'session_inference', 'observation'). Defaults to 'mcp_session'."
                        },
                        "confidence": {
                            "type": "number",
                            "description": "Confidence in this belief (0.0–1.0). Defaults to 0.7."
                        }
                    },
                    "required": ["content"]
                },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_session_status",
                "description": "Show the current Left Brain session memory state: active session facts, promotion candidates, recent queries, and active goal. Use to inspect what temporary beliefs are influencing this session's reasoning.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_session_promote",
                "description": "Propose promoting a session fact to permanent Right Brain knowledge. Creates a standard LearningProposal that must be approved by a human via brain_approve. THE WALL: this fact does NOT become permanent knowledge until brain_approve is called. Returns the proposal ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "fact_id": {
                            "type": "string",
                            "description": "The session fact ID to propose for promotion (from brain_session_status)"
                        }
                    },
                    "required": ["fact_id"]
                },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_session_clear",
                "description": "Wipe all current session memory: context, facts, and promotion candidates. The Left Brain starts fresh. Permanent Right Brain knowledge is unaffected.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_autonomy_status",
                "description": "Show current autonomy engine status: execution tier assignments for each action type, queued actions awaiting execution, and recent execution history.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_autonomy_elevate",
                "description": "Elevate (or lower) an action type to a different execution tier. Tiers: Observe (read-only), Suggest (needs approval), Act (auto-execute), Create (full autonomy). Action types: Recall, CreateSubGoal, FormHypothesis, PromoteBond, LearnInsight, Revoke, StrengthenBond, WeakenBond, CreateNode, DeleteNode.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action_type": {
                            "type": "string",
                            "description": "The action type to change (e.g. 'FormHypothesis', 'CreateSubGoal')"
                        },
                        "tier": {
                            "type": "string",
                            "description": "The new tier: Observe, Suggest, Act, or Create"
                        }
                    },
                    "required": ["action_type", "tier"]
                },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_autonomy_execute",
                "description": "Try to execute the next queued action. If the action's tier allows auto-execution, it runs. If not, returns Blocked with the reason. The caller must still perform the actual graph mutation for Success results.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_autonomy_policy",
                "description": "View the current execution policy: which action types are at which tier.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_curiosity_status",
                "description": "View the curiosity engine status: pending wonders, interest profile, gap stats.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_curiosity_trigger",
                "description": "Manually trigger curiosity analysis on a query and domain. Runs recall + gap detection.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Query to analyse for knowledge gaps" },
                        "domain": { "type": "string", "description": "Knowledge domain (e.g. 'physics', 'history')" }
                    },
                    "required": ["query", "domain"]
                },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_wonder_queue",
                "description": "Peek at the top N items in the wonder queue (things Lu is most curious about).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "count": { "type": "integer", "description": "How many wonders to show (default 10)" }
                    },
                    "required": []
                },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_generate_variants",
                "description": "Generate competing hypothesis variants for a query. Spawns N variants with different perturbation strategies, runs activation, and scores them.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Query to generate variants for" },
                        "count": { "type": "integer", "description": "Number of variants (3-5 recommended, max 10)" }
                    },
                    "required": ["query"]
                },
                "annotations": { "readOnlyHint": false, "destructiveHint": false }
            },
            {
                "name": "brain_variant_status",
                "description": "View the current variant arena state and scores.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] },
                "annotations": { "readOnlyHint": true, "destructiveHint": false }
            },
            {
                "name": "brain_variant_promote",
                "description": "Promote top N variants from the arena to proposals for human review.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "count": { "type": "integer", "description": "How many to promote (default 2)" }
                    },
                    "required": []
                },
                "annotations": { "readOnlyHint": false, "destructiveHint": false }
            },
            {
                "name": "brain_self_improvement_status",
                "description": "View self-improvement engine: performance metrics, gap trends, meta-reflections.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] },
                "annotations": { "readOnlyHint": true, "destructiveHint": false }
            },
            {
                "name": "brain_meta_reflect",
                "description": "Run meta-reflection: analyse performance patterns and propose heuristic adjustments.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] },
                "annotations": { "readOnlyHint": false, "destructiveHint": false }
            },
            {
                "name": "brain_invention_status",
                "description": "View cross-domain bridge detection status: recent bridges, domain pairs discovered.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] },
                "annotations": { "readOnlyHint": true, "destructiveHint": false }
            },
            {
                "name": "brain_health_report",
                "description": "Knowledge graph health dashboard: density, orphans, hubs, coherence, domain count.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] },
                "annotations": { "readOnlyHint": true, "destructiveHint": false }
            },
            {
                "name": "brain_synthesis_pass",
                "description": "Run a synthesis pass: find knowledge triangles (A→B, B→C) where A→C is missing, and propose the bond.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] },
                "annotations": { "readOnlyHint": true, "destructiveHint": false }
            },
            {
                "name": "brain_curiosity_configure",
                "description": "Configure the curiosity engine: enable/disable, toggle auto-resolve, or set domain interest weights. auto_resolve=true makes Lu automatically research gaps detected during recall() and commit the findings.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "enabled": { "type": "boolean", "description": "Enable or disable curiosity gap detection" },
                        "auto_resolve": { "type": "boolean", "description": "Enable autonomous gap-filling: when recall hits a sparse result, Lu auto-searches the web, ingests facts, and approves up to 20 nodes. Runs after the response is sent so it never blocks. Default: false." },
                        "domain": { "type": "string", "description": "Domain to set interest for" },
                        "weight": { "type": "number", "description": "Interest weight 0.0-1.0" }
                    },
                    "required": []
                },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_auto_resolve_wonder",
                "description": "Manually trigger one autonomous wonder-resolution cycle. Pops the highest-priority gap from the wonder queue, searches the web, ingests facts, auto-approves up to 20 nodes. Use this to trigger on-demand learning without waiting for a sparse recall to activate it.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false
                }
            },
            {
                "name": "brain_orchestrate",
                "description": "The smart front door to Lu. Takes raw user input and intelligently routes it to the right subsystem (question answering, reasoning, thinking, tool actions, learning, etc). Returns a clean conversational response. No /commands needed - just talk to Lu.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Raw user input - anything they'd say to Lu" }
                    },
                    "required": ["input"]
                }
            },
            {
                "name": "brain_tool_status",
                "description": "Get the status of the tool integration engine: pending requests, history, custom tools.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] },
                "annotations": { "readOnlyHint": true }
            },
            {
                "name": "brain_tool_detect",
                "description": "Analyse a query for tool intent (file read/write, web fetch, shell command). Returns detected intent and request ID if action is implied.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "The user query to analyse for tool intent" }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "brain_tool_approve",
                "description": "Approve a pending tool request by ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "Tool request ID to approve" }
                    },
                    "required": ["id"]
                },
                "annotations": { "destructiveHint": false }
            },
            {
                "name": "brain_tool_reject",
                "description": "Reject a pending tool request by ID with a reason.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "Tool request ID to reject" },
                        "reason": { "type": "string", "description": "Why the request is rejected" }
                    },
                    "required": ["id"]
                }
            },
            {
                "name": "brain_tool_pending",
                "description": "List all tool requests awaiting human approval.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] },
                "annotations": { "readOnlyHint": true }
            },
            {
                "name": "brain_tool_ready",
                "description": "List all tool requests that are approved and ready to execute.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] },
                "annotations": { "readOnlyHint": true }
            },
            {
                "name": "brain_approve_all",
                "description": "Approve ALL pending learning proposals at once. Skips any with scar conflicts. Returns count of approved facts.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] }
            },
            {
                "name": "brain_web_research",
                "description": "Lu researches a topic on the web. Searches DuckDuckGo, visits pages, follows links, extracts knowledge, and ingests what it learns. Give Lu a question and it will go find the answer.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "What to research (e.g. 'What is DialectForge?')" },
                        "ingest": { "type": "boolean", "description": "Auto-ingest findings into the brain (default true)" }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "brain_web_browse",
                "description": "Lu browses a specific URL. Reads the page, follows promising links, extracts knowledge. Use when you have a specific URL for Lu to visit.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "The URL to visit" },
                        "query": { "type": "string", "description": "Context for what to look for on the page" },
                        "ingest": { "type": "boolean", "description": "Auto-ingest findings into the brain (default true)" }
                    },
                    "required": ["url"]
                }
            },
            {
                "name": "brain_web_idle_status",
                "description": "Check Lu's idle exploration status — is it exploring? How long idle? What has it learned while you were away?",
                "inputSchema": { "type": "object", "properties": {}, "required": [] },
                "annotations": { "readOnlyHint": true }
            },
            {
                "name": "brain_web_idle_toggle",
                "description": "Enable or disable Lu's idle exploration (auto-browsing when idle).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "enabled": { "type": "boolean", "description": "true to enable, false to disable" }
                    },
                    "required": ["enabled"]
                }
            },
            {
                "name": "brain_cleanup_glossary",
                "description": "Clean up glossary/appendix/index noise nodes from the knowledge graph. Identifies and revokes: nodes with glossary/appendix/index prefixes, empty labels, and short labels with extremely high bond counts (index noise). Returns a report of what was cleaned.",
                "inputSchema": { "type": "object", "properties": {} },
                "annotations": { "destructiveHint": true }
            },
            {
                "name": "brain_code_generate",
                "description": "Generate a code block for a task. Returns a CodeBlock with language, code, and description. The code is NOT executed — use brain_code_execute to run it.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "description": { "type": "string", "description": "What the code should do" },
                        "language": { "type": "string", "description": "Target language: python, rust, or powershell (default: python)" },
                        "code": { "type": "string", "description": "The source code to include in the block" }
                    },
                    "required": ["description", "code"]
                }
            },
            {
                "name": "brain_code_execute",
                "description": "Execute code in a sandboxed environment. Runs in a temp directory with timeout. Returns stdout, stderr, exit code, duration, and files created.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "language": { "type": "string", "description": "Language: python, rust, or powershell" },
                        "code": { "type": "string", "description": "Source code to execute" },
                        "description": { "type": "string", "description": "What the code does (optional)" }
                    },
                    "required": ["language", "code"]
                },
                "annotations": { "readOnlyHint": false }
            },
            {
                "name": "brain_file_read",
                "description": "Read a file from the sandbox directory. Only files inside the sandbox can be read.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "filename": { "type": "string", "description": "File path relative to sandbox directory" }
                    },
                    "required": ["filename"]
                },
                "annotations": { "readOnlyHint": true }
            },
            {
                "name": "brain_file_write",
                "description": "Write a file to the sandbox directory. Creates or overwrites the file.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "filename": { "type": "string", "description": "File path relative to sandbox directory" },
                        "content": { "type": "string", "description": "File content to write" }
                    },
                    "required": ["filename", "content"]
                }
            },
            {
                "name": "brain_file_list",
                "description": "List files in the sandbox directory. Returns name, type (file/dir), and size for each entry.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "subdir": { "type": "string", "description": "Subdirectory to list (optional, defaults to sandbox root)" }
                    }
                },
                "annotations": { "readOnlyHint": true }
            },
            {
                "name": "brain_think_from_wonder",
                "description": "Start a thinking session from Lu's top curiosity wonder. Pops the highest-priority knowledge gap and uses it as the thinking goal.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "max_iterations": {
                            "type": "integer",
                            "description": "Maximum thinking iterations (default 5, max 20)",
                            "default": 5
                        },
                        "coherence_threshold": {
                            "type": "number",
                            "description": "Coherence score at which thinking is done (default 0.6)",
                            "default": 0.6
                        },
                        "dry_run": {
                            "type": "boolean",
                            "description": "Preview mode — no real nodes/bonds created (default false)",
                            "default": false
                        },
                        "auto_approve_safe": {
                            "type": "boolean",
                            "description": "Auto-approve non-destructive proposals (default false)",
                            "default": false
                        }
                    }
                },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false,
                    "idempotentHint": false
                }
            },
            {
                "name": "brain_cleanup",
                "description": "Scan all active nodes and identify junk accumulated from PDF textbook training: page numbers, TOC entries, index references, copyright notices, exercise prompts, formatting artifacts, and too-short labels. By default runs as a dry run (execute=false) — set execute=true to actually revoke the flagged nodes. Always dry-run first to review what will be removed.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "execute": {
                            "type": "boolean",
                            "description": "If true, revoke all flagged nodes. If false (default), report only — nothing is removed.",
                            "default": false
                        },
                        "min_label_length": {
                            "type": "integer",
                            "description": "Minimum label character length to keep (default 10). Nodes shorter than this are flagged as too_short.",
                            "default": 10
                        }
                    }
                },
                "annotations": {
                    "destructiveHint": true
                }
            }
        ]
    });
    protocol::success_response(id.clone(), tools)
}

/// Dispatch a tools/call request to the appropriate handler.
pub fn handle_call(id: &Value, params: &Value, brain: &mut Brain) -> String {
    let tool_name = params.get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("");
    let args = params.get("arguments")
        .cloned()
        .unwrap_or(json!({}));

    eprintln!("[lu_ai_mcp] tool={} args={}", tool_name, args);

    match tool_name {
        "brain_learn" => tool_learn(id, &args, brain),
        "brain_recall" => tool_recall(id, &args, brain),
        "brain_associate" => tool_associate(id, &args, brain),
        "brain_revoke" => tool_revoke(id, &args, brain),
        "brain_propose" => tool_propose(id, &args, brain),
        "brain_approve" => tool_approve(id, &args, brain),
        "brain_reject" => tool_reject(id, &args, brain),
        "brain_stats" => tool_stats(id, brain),
        "brain_scars" => tool_scars(id, brain),
        "brain_seed_report" => tool_seed_report(id, brain),
        "brain_batch_train" => tool_batch_train(id, &args, brain),
        "brain_session_report" => tool_session_report(id, &args, brain),
        "brain_self_reflect" => tool_self_reflect(id, brain),
        "brain_decay" => tool_decay(id, &args, brain),
        "brain_prune" => tool_prune(id, brain),
        "brain_self_learn" => tool_self_learn(id, &args, brain),
        "brain_strengthen_bond" => tool_strengthen_bond(id, &args, brain),
        "brain_weaken_bond" => tool_weaken_bond(id, &args, brain),
        "brain_respond" => tool_respond(id, &args, brain),
        "brain_ingest_text" => tool_ingest_text(id, &args, brain),
        "brain_ingest_file" => tool_ingest_file(id, &args, brain),
        "brain_approve_ingest" => tool_approve_ingest(id, &args, brain),
        "brain_save" => tool_save(id, brain),
        "brain_resize" => tool_resize(id, &args, brain),
        "brain_reason" => tool_reason(id, &args, brain),
        "brain_reindex" => tool_reindex(id, &args, brain),
        "brain_ask" => tool_ask(id, &args, brain),
        "brain_think" => tool_think(id, &args, brain),
        "brain_think_step" => tool_think_step(id, brain),
        "brain_think_approve" => tool_think_approve(id, &args, brain),
        "brain_think_reject" => tool_think_reject(id, &args, brain),
        "brain_think_status" => tool_think_status(id, brain),
        "brain_think_stop" => tool_think_stop(id, brain),
        "brain_think_simulate" => tool_think_simulate(id, &args, brain),
        "brain_approve_media" => tool_approve_media(id, &args, brain),
        "brain_reject_media" => tool_reject_media(id, &args, brain),
        "brain_cross_modal_search" => tool_cross_modal_search(id, &args, brain),
        "brain_session_context" => tool_session_context(id, &args, brain),
        "brain_session_status" => tool_session_status(id, brain),
        "brain_session_promote" => tool_session_promote(id, &args, brain),
        "brain_session_clear" => tool_session_clear(id, brain),
        "brain_autonomy_status" => tool_autonomy_status(id, brain),
        "brain_autonomy_elevate" => tool_autonomy_elevate(id, &args, brain),
        "brain_autonomy_execute" => tool_autonomy_execute(id, brain),
        "brain_autonomy_policy" => tool_autonomy_policy(id, brain),
        "brain_self_improvement_status" => tool_self_improvement_status(id, brain),
        "brain_meta_reflect" => tool_meta_reflect(id, brain),
        "brain_invention_status" => tool_invention_status(id, brain),
        "brain_health_report" => tool_health_report(id, brain),
        "brain_synthesis_pass" => tool_synthesis_pass(id, brain),
        "brain_generate_variants" => tool_generate_variants(id, &args, brain),
        "brain_variant_status" => tool_variant_status(id, brain),
        "brain_variant_promote" => tool_variant_promote(id, &args, brain),
        "brain_curiosity_status" => tool_curiosity_status(id, brain),
        "brain_curiosity_trigger" => tool_curiosity_trigger(id, &args, brain),
        "brain_wonder_queue" => tool_wonder_queue(id, &args, brain),
        "brain_curiosity_configure" => tool_curiosity_configure(id, &args, brain),
        "brain_auto_resolve_wonder" => tool_auto_resolve_wonder(id, brain),
        "brain_orchestrate" => tool_orchestrate(id, &args, brain),
        "brain_tool_status" => tool_tool_status(id, brain),
        "brain_tool_detect" => tool_tool_detect(id, &args, brain),
        "brain_tool_approve" => tool_tool_approve(id, &args, brain),
        "brain_tool_reject" => tool_tool_reject(id, &args, brain),
        "brain_tool_pending" => tool_tool_pending(id, brain),
        "brain_tool_ready" => tool_tool_ready(id, brain),
        "brain_approve_all" => tool_approve_all(id, brain),
        "brain_web_research" => tool_web_research(id, &args, brain),
        "brain_web_browse" => tool_web_browse(id, &args, brain),
        "brain_web_idle_status" => tool_web_idle_status(id, brain),
        "brain_web_idle_toggle" => tool_web_idle_toggle(id, &args, brain),
        "brain_cleanup_glossary" => tool_cleanup_glossary(id, brain),
        "brain_code_generate" => tool_code_generate(id, &args, brain),
        "brain_code_execute" => tool_code_execute(id, &args, brain),
        "brain_file_read" => tool_file_read(id, &args, brain),
        "brain_file_write" => tool_file_write(id, &args, brain),
        "brain_file_list" => tool_file_list(id, &args, brain),
        "brain_think_from_wonder" => tool_think_from_wonder(id, &args, brain),
        "brain_cleanup" => tool_brain_cleanup(id, &args, brain),
        _ => protocol::tool_error(id, &format!("Unknown tool: {}", tool_name)),
    }
}


// ====== Tool Handlers ======

fn get_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn get_u64(args: &Value, key: &str, default: u64) -> u64 {
    args.get(key).and_then(|v| v.as_u64()).unwrap_or(default)
}

fn tool_learn(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let content = match get_str(args, "content") {
        Some(c) => c,
        None => return protocol::tool_error(id, "Missing required parameter: content"),
    };
    let seed = brain.learn(content, graph::Confidence::Terminal, "mcp_claude");
    let _ = brain.save();
    protocol::tool_result(id, &format!(
        "Learned. Seed: {}\nContent: {}",
        seed.to_hex(), content
    ))
}

fn tool_recall(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let query = match get_str(args, "query") {
        Some(q) => q,
        None => return protocol::tool_error(id, "Missing required parameter: query"),
    };
    let depth = get_u64(args, "depth", 3) as usize;
    let top_k = get_u64(args, "top_k", 10) as usize;

    let result = brain.recall(query, depth, top_k);
    // Update conversation history so subsequent recall() calls get context injection.
    // (ask() does this itself; tool_recall calls recall() directly so we do it here.)
    brain.push_conversation_history(query);

    // Save topic seeds for next turn (ask() does this itself; tool_recall must too).
    // Filter out hub nodes (bond_count > 500) — high activation from connectivity, not relevance.
    {
        use brain::session_memory::TopicSeed;
        let top_seeds: Vec<TopicSeed> = result.activated_nodes.iter()
            .filter(|n| {
                brain.bond_count_for(&n.seed_hex)
                    .map_or(true, |c| c <= 500)
            })
            .take(10)
            .map(|n| TopicSeed {
                seed_hex: n.seed_hex.clone(),
                score: n.strength,
            })
            .collect();
        brain.session_memory_set_topic_seeds(top_seeds);
    }

    let output = json!({
        "query": query,
        "activated_nodes": result.activated_nodes.iter().map(|n| {
            json!({
                "seed": n.seed_hex,
                "label": n.label,
                "strength": n.strength,
                "contribution_pct": (n.contribution * 100.0),
                "confidence": format!("{:?}", n.confidence)
            })
        }).collect::<Vec<_>>(),
        "traversal_hops": result.traversal_path.len(),
        "scar_warnings": result.scar_warnings
    });

    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_associate(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let seed_a_hex = match get_str(args, "seed_a") {
        Some(s) => s,
        None => return protocol::tool_error(id, "Missing required parameter: seed_a"),
    };
    let seed_b_hex = match get_str(args, "seed_b") {
        Some(s) => s,
        None => return protocol::tool_error(id, "Missing required parameter: seed_b"),
    };
    let desc = get_str(args, "description").unwrap_or("association");
    let weight = args.get("weight")
        .and_then(|v| v.as_f64())
        .map(|w| w as f32)
        .unwrap_or(1.0);

    let seed_a = match Seed::from_hex(seed_a_hex) {
        Ok(s) => s,
        Err(_) => return protocol::tool_error(id, &format!("Invalid seed_a: {}", seed_a_hex)),
    };
    let seed_b = match Seed::from_hex(seed_b_hex) {
        Ok(s) => s,
        Err(_) => return protocol::tool_error(id, &format!("Invalid seed_b: {}", seed_b_hex)),
    };

    match brain.associate(&seed_a, &seed_b, desc, weight, "mcp_claude") {
        Ok(bond_seed) => {
            let _ = brain.save();
            protocol::tool_result(id, &format!(
                "Bond created. Seed: {}\n{} <-> {}",
                bond_seed.to_hex(), seed_a_hex, seed_b_hex
            ))
        }
        Err(e) => protocol::tool_error(id, &e),
    }
}

fn tool_revoke(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let seed_hex = match get_str(args, "seed") {
        Some(s) => s,
        None => return protocol::tool_error(id, "Missing required parameter: seed"),
    };
    let reason = get_str(args, "reason").unwrap_or("no reason given");

    let seed = match Seed::from_hex(seed_hex) {
        Ok(s) => s,
        Err(_) => return protocol::tool_error(id, &format!("Invalid seed: {}", seed_hex)),
    };

    let report = brain.revoke(&seed, reason);
    let _ = brain.save();

    let output = json!({
        "revoked_seed": report.revoked_seed,
        "entity_type": report.entity_type,
        "label": report.label,
        "scar_created": report.scar_created,
        "children_to_review": report.children_to_review
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_propose(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let content = match get_str(args, "content") {
        Some(c) => c,
        None => return protocol::tool_error(id, "Missing required parameter: content"),
    };
    let source = get_str(args, "source").unwrap_or("mcp_claude");

    let proposal = brain.propose(content, source);
    let output = json!({
        "proposal_id": proposal.id,
        "content": proposal.content,
        "recommendation": proposal.recommendation,
        "similar_existing": proposal.similar_existing.iter().map(|(s, l, sim)| {
            json!({"seed": s, "label": l, "similarity": sim})
        }).collect::<Vec<_>>(),
        "scar_conflicts": proposal.scar_conflicts
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_approve(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let pid = match get_str(args, "proposal_id") {
        Some(p) => p,
        None => return protocol::tool_error(id, "Missing required parameter: proposal_id"),
    };
    match brain.approve_proposal(pid) {        Some(seed) => {
            let _ = brain.save();
            protocol::tool_result(id, &format!("Approved. Seed: {}", seed.to_hex()))
        }
        None => protocol::tool_error(id, &format!("No pending proposal: {}", pid)),
    }
}

fn tool_reject(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let pid = match get_str(args, "proposal_id") {
        Some(p) => p,
        None => return protocol::tool_error(id, "Missing required parameter: proposal_id"),
    };
    if brain.reject_proposal(pid) {
        protocol::tool_result(id, &format!("Proposal {} rejected.", pid))
    } else {
        protocol::tool_error(id, &format!("No pending proposal: {}", pid))
    }
}

fn tool_stats(id: &Value, brain: &Brain) -> String {
    let s = brain.stats();
    let output = json!({
        "active_nodes": s.active_nodes,
        "revoked_nodes": s.revoked_nodes,
        "active_bonds": s.active_bonds,
        "dead_bonds": s.dead_bonds,
        "scars": s.scars,
        "total_nodes": s.total_nodes,
        "total_bonds": s.total_bonds,
        "terminal_bond_state": format!("{:?}", brain.terminal_bond().state()),
        "terminal_bond_exchanges": brain.terminal_bond().exchange_count()
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_scars(id: &Value, brain: &Brain) -> String {
    let scars = brain.scars();
    if scars.is_empty() {
        return protocol::tool_result(id, "No scars. No revocations have occurred.");
    }
    let output: Vec<Value> = scars.iter().map(|scar| {
        json!({
            "seed": scar.revoked_seed.short(),
            "label": scar.label,
            "reason": scar.reason,
            "revoked_at": scar.revoked_at.to_rfc3339(),
            "fingerprint_dims": scar.pattern_fingerprint.len()
        })
    }).collect();
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_seed_report(id: &Value, brain: &Brain) -> String {
    match brain.last_seed_report() {
        Some(result) => {
            let output: Vec<Value> = result.activated_nodes.iter().map(|n| {
                json!({
                    "seed": n.seed_hex,
                    "label": n.label,
                    "contribution_pct": (n.contribution * 100.0),
                    "confidence": format!("{:?}", n.confidence)
                })
            }).collect();
            protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        None => protocol::tool_result(id, "No recent recall to report on."),
    }
}

fn tool_batch_train(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let items = match args.get("items").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return protocol::tool_error(id, "Missing required parameter: items (array)"),
    };

    let mut results: Vec<Value> = Vec::new();
    let mut learned_seeds: std::collections::HashMap<String, Seed> = std::collections::HashMap::new();
    let mut total_learned = 0usize;
    let mut total_bonds = 0usize;

    for item in items {
        let content = match item.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                results.push(json!({"error": "Missing content field"}));
                continue;
            }
        };

        // Learn the fact (fast -- skip auto_associate, we have explicit associations)
        let seed = brain.learn_fast(content, graph::Confidence::Terminal, "mcp_batch");
        learned_seeds.insert(content.to_string(), seed.clone());
        total_learned += 1;

        let mut item_result = json!({
            "content": content,
            "seed": seed.to_hex(),
            "bonds_created": 0
        });

        // Process explicit associations
        if let Some(assocs) = item.get("associations").and_then(|v| v.as_array()) {
            let mut bonds = 0;
            for assoc_val in assocs {
                if let Some(assoc_content) = assoc_val.as_str() {
                    // Check if we already learned this association target
                    let target_seed = if let Some(existing) = learned_seeds.get(assoc_content) {
                        existing.clone()
                    } else {
                        // Learn the association target too (fast)
                        let s = brain.learn_fast(assoc_content, graph::Confidence::Terminal, "mcp_batch");
                        learned_seeds.insert(assoc_content.to_string(), s.clone());
                        total_learned += 1;
                        s
                    };

                    // Create the bond
                    if let Ok(_) = brain.associate(&seed, &target_seed, "batch_association", 1.0, "mcp_batch") {
                        bonds += 1;
                        total_bonds += 1;
                    }
                }
            }
            item_result["bonds_created"] = json!(bonds);
        }

        results.push(item_result);
    }

    // Save after batch
    let _ = brain.save();

    let output = json!({
        "total_learned": total_learned,
        "total_bonds": total_bonds,
        "items": results
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_session_report(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let items = match args.get("items").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return protocol::tool_error(id, "Missing required parameter: items (array)"),
    };

    let report_items: Vec<brain::SessionReportItem> = items.iter()
        .filter_map(|item| {
            let content = item.get("content").and_then(|v| v.as_str())?;
            let associations = item.get("associations")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect());
            Some(brain::SessionReportItem {
                content: content.to_string(),
                associations,
            })
        })
        .collect();

    let report = brain.session_report(&report_items);

    let entries: Vec<Value> = report.entries.iter().map(|e| {
        json!({
            "content": e.content,
            "status": e.status,
            "similar_existing": e.similar_existing.iter().map(|(s, l, sim)| {
                json!({"seed": &s[..16], "label": l, "similarity": format!("{:.2}", sim)})
            }).collect::<Vec<_>>(),
            "scar_conflicts": e.scar_conflicts,
            "associations": e.association_targets,
            "bond_count": e.bond_count,
        })
    }).collect();

    let output = json!({
        "summary": {
            "total_items": report.total_items,
            "new_atoms": report.total_new_atoms,
            "new_bonds": report.total_new_bonds,
            "scar_warnings": report.total_scar_warnings,
            "current_nodes": report.current_brain_nodes,
            "current_bonds": report.current_brain_bonds,
            "projected_nodes": report.projected_nodes,
            "projected_bonds": report.projected_bonds,
        },
        "entries": entries
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

// ====== Self-Modification Tools (v0 7.6) ======

fn tool_self_reflect(id: &Value, brain: &mut Brain) -> String {
    let report = brain.self_reflect();
    let _ = brain.save();

    let output = json!({
        "bonds_strengthened": report.bonds_strengthened,
        "bonds_created": report.bonds_created,
        "novel_associations": report.novel_associations.iter().map(|(a, b)| {
            json!({"node_a": a, "node_b": b})
        }).collect::<Vec<_>>(),
        "strengthened_pairs": report.strengthened_pairs.iter().map(|(a, b, w)| {
            json!({"node_a": a, "node_b": b, "new_weight": w})
        }).collect::<Vec<_>>()
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_decay(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let amount = args.get("amount")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.01) as f32;

    let (decayed, killed) = brain.decay(amount);
    let _ = brain.save();

    let output = json!({
        "bonds_decayed": decayed,
        "bonds_killed": killed,
        "decay_amount": amount
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_prune(id: &Value, brain: &mut Brain) -> String {
    let pruned = brain.prune();
    let _ = brain.save();
    protocol::tool_result(id, &format!("Pruned {} dead bonds from the network.", pruned))
}

fn tool_self_learn(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let content = match get_str(args, "content") {
        Some(c) => c,
        None => return protocol::tool_error(id, "Missing required parameter: content"),
    };

    let parent_hexes: Vec<String> = args.get("parent_seeds")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect())
        .unwrap_or_default();

    match brain.self_learn(content, &parent_hexes) {
        Some(seed) => {
            let _ = brain.save();
            let output = json!({
                "status": "learned",
                "seed": seed.to_hex(),
                "content": content,
                "parent_count": parent_hexes.len()
            });
            protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        None => {
            protocol::tool_result(id, &json!({
                "status": "blocked",
                "reason": "Scar conflict or duplicate content prevented self-learning"
            }).to_string())
        }
    }
}

fn tool_strengthen_bond(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let seed_a = match get_str(args, "seed_a") {
        Some(s) => s,
        None => return protocol::tool_error(id, "Missing seed_a"),
    };
    let seed_b = match get_str(args, "seed_b") {
        Some(s) => s,
        None => return protocol::tool_error(id, "Missing seed_b"),
    };
    let amount = args.get("amount")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.1) as f32;

    match brain.strengthen_bond(seed_a, seed_b, amount) {
        Ok(new_weight) => {
            let _ = brain.save();
            protocol::tool_result(id, &format!(
                "Bond strengthened. New weight: {:.2}", new_weight
            ))
        }
        Err(e) => protocol::tool_error(id, &e),
    }
}

fn tool_weaken_bond(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let seed_a = match get_str(args, "seed_a") {
        Some(s) => s,
        None => return protocol::tool_error(id, "Missing seed_a"),
    };
    let seed_b = match get_str(args, "seed_b") {
        Some(s) => s,
        None => return protocol::tool_error(id, "Missing seed_b"),
    };
    let amount = args.get("amount")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.1) as f32;

    match brain.weaken_bond(seed_a, seed_b, amount) {
        Ok(new_weight) => {
            let _ = brain.save();
            protocol::tool_result(id, &format!(
                "Bond weakened. New weight: {:.2}", new_weight
            ))
        }
        Err(e) => protocol::tool_error(id, &e),
    }
}

// ====== Conversational Prompt Interface (v0 7.8) ======

fn tool_respond(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let query = match get_str(args, "query") {
        Some(q) => q,
        None => return protocol::tool_error(id, "Missing required parameter: query"),
    };
    let depth = get_u64(args, "depth", 3) as usize;
    let top_k = get_u64(args, "top_k", 10) as usize;

    let response = brain.respond(query, depth, top_k);
    let _ = brain.save();

    let output = json!({
        "response": response.text,
        "query": response.query,
        "atoms_used": response.atoms_used,
        "traversal_hops": response.traversal_hops,
        "coherence_score": response.coherence_score,
        "sources": response.sources.iter().map(|(seed, label, pct)| {
            json!({
                "seed": &seed[..16.min(seed.len())],
                "label": label,
                "contribution_pct": format!("{:.1}%", pct)
            })
        }).collect::<Vec<_>>(),
        "scar_warnings": response.scar_warnings,
        "images": response.image_urls.iter().map(|(url, desc)| {
            json!({ "url": url, "description": desc })
        }).collect::<Vec<_>>()
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

// ====== External Data Ingestion Tools (v0 7.7) ======

fn tool_ingest_text(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let text = match get_str(args, "text") {        Some(t) => t,
        None => return protocol::tool_error(id, "Missing required parameter: text"),
    };
    let source = get_str(args, "source").unwrap_or("unknown");

    let report = brain.ingest_text(text, source);

    let proposals: Vec<Value> = report.proposals.iter().map(|p| {
        json!({
            "id": p.id,
            "content": p.content,
            "status": p.status,
            "scar_conflicts": p.scar_conflicts,
            "similar_existing": p.similar_existing.iter().map(|(s, l, sim)| {
                json!({"seed": &s[..16.min(s.len())], "label": l, "similarity": format!("{:.2}", sim)})
            }).collect::<Vec<_>>()
        })
    }).collect();

    let output = json!({
        "source": report.source,
        "total_sentences": report.total_sentences,
        "proposals_ready": report.proposals_ready,
        "proposals_blocked": report.proposals_blocked,
        "duplicates_skipped": report.duplicates_skipped,
        "proposals": proposals
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_ingest_file(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let path = match get_str(args, "path") {
        Some(p) => p,
        None => return protocol::tool_error(id, "Missing required parameter: path"),
    };

    match brain.ingest_file(path) {
        Ok(report) => {
            let proposals: Vec<Value> = report.proposals.iter().map(|p| {
                json!({
                    "id": p.id,
                    "content": p.content,
                    "status": p.status,
                    "scar_conflicts": p.scar_conflicts,
                    "similar_existing": p.similar_existing.iter().map(|(s, l, sim)| {
                        json!({"seed": &s[..16.min(s.len())], "label": l, "similarity": format!("{:.2}", sim)})
                    }).collect::<Vec<_>>()
                })
            }).collect();

            let output = json!({
                "source": report.source,
                "total_sentences": report.total_sentences,
                "proposals_ready": report.proposals_ready,
                "proposals_blocked": report.proposals_blocked,
                "duplicates_skipped": report.duplicates_skipped,
                "proposals": proposals
            });
            protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        Err(e) => protocol::tool_error(id, &e),
    }
}

fn tool_approve_ingest(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let proposal_ids: Vec<String> = match args.get("proposal_ids").and_then(|v| v.as_array()) {
        Some(arr) => arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        None => return protocol::tool_error(id, "Missing required parameter: proposal_ids (array)"),
    };

    let results = brain.approve_all_ready(&proposal_ids);
    let _ = brain.save();

    let learned: Vec<Value> = results.iter().map(|(pid, seed)| {
        json!({
            "proposal_id": pid,
            "seed": seed.to_hex()
        })
    }).collect();

    let output = json!({
        "approved_count": results.len(),
        "requested_count": proposal_ids.len(),
        "learned": learned
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_save(id: &Value, brain: &Brain) -> String {
    // save_full() writes bincode + JSON backup. Slow at large graph sizes
    // but this is an explicit user request, so correctness wins over speed.
    match brain.save_full() {
        Ok(()) => protocol::tool_result(id, "Brain state saved (bincode + JSON backup)."),
        Err(e) => protocol::tool_error(id, &format!("Save failed: {}", e)),
    }
}

fn tool_resize(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let new_size = get_u64(args, "dimensions", 256) as usize;
    if new_size < 64 || new_size > 1024 {
        return protocol::tool_error(id, "Dimensions must be between 64 and 1024");
    }
    let result = brain.resize_dimensions(new_size);
    let _ = brain.save();
    protocol::tool_result(id, &result)
}

// ====== Conversational Ask (Narration Engine) ======

fn tool_ask(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let query = match get_str(args, "query") {
        Some(q) => q,
        None => return protocol::tool_error(id, "Missing required parameter: query"),
    };
    let depth = get_u64(args, "depth", 3) as usize;
    let top_k = get_u64(args, "top_k", 10) as usize;

    let response = brain.ask(query, depth, top_k);

    // Persist the exchange to ConversationStore so conversation_history
    // survives process restarts (restored in Brain::load()).
    {
        let session_id = brain.ensure_active_session();
        let _ = brain.add_conversation_message(
            &session_id,
            brain::conversations::MessageRole::User,
            query,
            vec![],
            vec![],
        );
        let _ = brain.add_conversation_message(
            &session_id,
            brain::conversations::MessageRole::Assistant,
            &response.text.clone(),
            vec![],
            response.sources.clone(),
        );
    }

    let _ = brain.save();

    let output = json!({
        "response": response.text,
        "query": response.query,
        "intent": format!("{:?}", "conversational"),
        "atoms_used": response.atoms_used,
        "traversal_hops": response.traversal_hops,
        "coherence_score": response.coherence_score,
        "knowledge_gap": response.knowledge_gap,
        "suggestion": response.suggestion,
        "sources": response.sources.iter().map(|(seed, label, pct)| {
            json!({
                "seed": &seed[..16.min(seed.len())],
                "label": label,
                "contribution_pct": format!("{:.1}%", pct)
            })
        }).collect::<Vec<_>>(),
        "scar_warnings": response.scar_warnings,
        "images": response.image_urls.iter().map(|(url, desc)| {
            json!({ "url": url, "description": desc })
        }).collect::<Vec<_>>()
    });

    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap())
}

fn tool_reason(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let query = match get_str(args, "query") {
        Some(q) => q,
        None => return protocol::tool_error(id, "Missing required parameter: query"),
    };
    let depth = get_u64(args, "depth", 3) as usize;

    let result = brain.reason(query, depth);

    let chains_json: Vec<serde_json::Value> = result.chains.iter().map(|chain| {
        let steps: Vec<serde_json::Value> = chain.steps.iter().map(|step| {
            json!({
                "label": step.label,
                "bond": step.bond_label,
                "strength": step.link_strength
            })
        }).collect();
        json!({
            "steps": steps,
            "confidence": chain.confidence,
            "synthesis": chain.synthesis
        })
    }).collect();

    let output = json!({
        "query": query,
        "direct_answer": result.direct_answer,
        "chains": chains_json,
        "conclusion": result.conclusion,
        "reasoning_depth": result.reasoning_depth
    });

    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}


// ====== Reindex ======

fn tool_reindex(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let max_freq_pct = args.get("max_freq_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(5.0) as f32;
    let max_per_word = args.get("max_per_word")
        .and_then(|v| v.as_u64())
        .unwrap_or(50) as usize;

    let (created, strengthened, words, skipped) =
        brain.reindex_bonds(max_freq_pct, max_per_word);

    let output = json!({
        "bonds_created": created,
        "bonds_strengthened": strengthened,
        "words_processed": words,
        "words_skipped_high_freq": skipped,
        "settings": {
            "max_freq_pct": max_freq_pct,
            "max_per_word": max_per_word
        }
    });

    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

// ━━━ Thinking Layer ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tool_think(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let goal = match get_str(args, "goal") {
        Some(g) => g,
        None => return protocol::tool_error(id, "Missing required parameter: goal"),
    };

    let config = coordinator::ThinkConfig {
        max_iterations: get_u64(args, "max_iterations", 5).min(20) as u32,
        coherence_threshold: args.get("coherence_threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.6) as f32,
        dry_run: args.get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        auto_approve_safe: args.get("auto_approve_safe")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    };

    match brain.think_start(goal, config) {
        Ok(state) => {
            let output = thinking_state_json(&state);
            protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        Err(e) => protocol::tool_error(id, &e),
    }
}

fn tool_think_step(id: &Value, brain: &mut Brain) -> String {
    match brain.think_step() {
        Ok(state) => {
            let output = thinking_state_json(&state);
            protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        Err(e) => protocol::tool_error(id, &e),
    }
}

fn tool_think_approve(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let proposal_id = match get_str(args, "proposal_id") {
        Some(p) => p,
        None => return protocol::tool_error(id, "Missing required parameter: proposal_id"),
    };
    match brain.think_approve(proposal_id) {
        Ok(msg) => protocol::tool_result(id, &msg),
        Err(e) => protocol::tool_error(id, &e),
    }
}

fn tool_think_reject(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let proposal_id = match get_str(args, "proposal_id") {
        Some(p) => p,
        None => return protocol::tool_error(id, "Missing required parameter: proposal_id"),
    };
    match brain.think_reject(proposal_id) {
        Ok(msg) => protocol::tool_result(id, &msg),
        Err(e) => protocol::tool_error(id, &e),
    }
}

fn tool_think_status(id: &Value, brain: &Brain) -> String {
    match brain.think_status() {
        Some(state) => {
            let output = thinking_state_json(&state);
            protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        None => protocol::tool_result(id, "No active thinking task."),
    }
}

fn tool_think_stop(id: &Value, brain: &mut Brain) -> String {
    match brain.think_stop() {
        Ok(summary) => {
            let observations: Vec<Value> = summary.observations.iter().map(|obs| {
                json!({
                    "iteration": obs.iteration,
                    "action": obs.action,
                    "outcome": obs.outcome,
                    "coherence_delta": obs.coherence_delta
                })
            }).collect();

            let output = json!({
                "goal": summary.goal,
                "iterations_completed": summary.iterations_completed,
                "final_coherence": summary.final_coherence,
                "nodes_created": summary.nodes_created,
                "bonds_formed": summary.bonds_formed,
                "nodes_promoted": summary.nodes_promoted,
                "nodes_cleaned": summary.nodes_cleaned,
                "observations": observations,
                "conclusion": summary.conclusion,
                "dry_run": summary.dry_run,
                "duration_secs": summary.duration_secs
            });
            protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        Err(e) => protocol::tool_error(id, &e),
    }
}

/// Convert ThinkingState to JSON for consistent output.
fn thinking_state_json(state: &coordinator::ThinkingState) -> Value {
    let proposals: Vec<Value> = state.pending_proposals.iter().map(|p| {
        json!({
            "id": p.id,
            "type": format!("{:?}", p.proposal_type),
            "description": p.description,
            "target_seeds": p.target_seeds,
            "scar_conflicts": p.scar_conflicts,
            "is_safe": p.is_safe()
        })
    }).collect();

    let last_obs = state.last_observation.as_ref().map(|obs| {
        json!({
            "iteration": obs.iteration,
            "action": obs.action,
            "outcome": obs.outcome,
            "coherence_delta": obs.coherence_delta
        })
    });

    json!({
        "goal": state.goal,
        "state": format!("{:?}", state.state),
        "iteration": state.iteration,
        "coherence": state.coherence,
        "sub_goal_count": state.sub_goal_count,
        "resolved_count": state.resolved_count,
        "pending_proposals": proposals,
        "last_observation": last_obs,
        "dry_run": state.dry_run
    })
}

// ====== Media Proposal Tools ======

fn tool_approve_media(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let pid = match get_str(args, "proposal_id") {
        Some(p) => p,
        None => return protocol::tool_error(id, "Missing required parameter: proposal_id"),
    };
    match brain.approve_media_proposal(pid) {
        Some(seed) => {
            let _ = brain.save();
            protocol::tool_result(id, &format!("Media approved. Seed: {}", seed.to_hex()))
        }
        None => protocol::tool_error(id, &format!(
            "No pending media proposal: {}. Use brain_ingest_file to create a proposal first.",
            pid
        )),
    }
}

fn tool_reject_media(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let pid = match get_str(args, "proposal_id") {
        Some(p) => p,
        None => return protocol::tool_error(id, "Missing required parameter: proposal_id"),
    };
    if brain.reject_media_proposal(pid) {
        protocol::tool_result(id, &format!("Media proposal {} rejected.", pid))
    } else {
        protocol::tool_error(id, &format!("No pending media proposal: {}", pid))
    }
}

// ====== Left Brain: Session Memory Tools ======

fn tool_session_context(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let content = match get_str(args, "content") {
        Some(c) => c,
        None => return protocol::tool_error(id, "Missing required parameter: content"),
    };
    let source = get_str(args, "source").unwrap_or("mcp_session");
    let confidence = args.get("confidence")
        .and_then(|v| v.as_f64())
        .map(|f| f as f32)
        .unwrap_or(0.7);

    let fact_id = brain.session_add_fact(content, source, confidence);
    protocol::tool_result(id, &format!(
        "Session fact added. id={fact_id}\nContent: {content}\nSource: {source}, confidence: {confidence:.2}\nNote: This is Left Brain working memory. It informs recall but will NOT auto-promote to permanent knowledge."
    ))
}

fn tool_session_status(id: &Value, brain: &mut Brain) -> String {
    let status = brain.session_status();
    let output = json!({
        "tasks": status.tasks.iter().map(|t| json!({
            "id": t.id,
            "description": t.description,
            "status": t.status,
            "parent_id": t.parent_id,
            "goal_seed": t.goal_seed,
            "attempt_count": t.attempt_count
        })).collect::<Vec<_>>(),
        "active_beliefs": status.active_beliefs,
        "promoted_count": status.promoted_count,
        "expired_count": status.expired_count,
        "task_count": status.task_count,
        "active_task_count": status.active_task_count,
        "attempt_count": status.attempt_count,
        "learning_count": status.learning_count,
        "active_goal": status.active_goal,
        "recent_queries": status.recent_queries,
        "beliefs": status.beliefs.iter().map(|b| json!({
            "id": b.id,
            "content": b.content,
            "source": b.source,
            "task_id": b.task_id,
            "confidence": b.confidence,
            "recall_count": b.recall_count,
            "is_learning_candidate": b.is_learning_candidate,
            "age_secs": b.age_secs,
            "ttl_secs": b.ttl_secs,
            "expires_in_secs": b.ttl_secs.saturating_sub(b.age_secs)
        })).collect::<Vec<_>>(),
        "learnings": status.learnings.iter().map(|l| json!({
            "id": l.id,
            "insight": l.insight,
            "category": l.category,
            "derived_from": l.derived_from
        })).collect::<Vec<_>>(),
        "recent_log": status.recent_log
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_session_promote(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let fact_id = match get_str(args, "fact_id") {
        Some(f) => f,
        None => return protocol::tool_error(id, "Missing required parameter: fact_id"),
    };
    match brain.session_propose_promote(fact_id) {
        Some(proposal) => {
            let output = json!({
                "proposal_id": proposal.id,
                "content": proposal.content,
                "source": proposal.source,
                "recommendation": proposal.recommendation,
                "similar_existing": proposal.similar_existing.iter().map(|(seed, label, sim)| json!({
                    "seed": seed,
                    "label": label,
                    "similarity": sim
                })).collect::<Vec<_>>(),
                "scar_conflicts": proposal.scar_conflicts,
                "next_step": format!("Call brain_approve with proposal_id='{}' to permanently add this to the Right Brain.", proposal.id)
            });
            protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        None => protocol::tool_error(id, &format!(
            "No promotable session fact with id '{}'. Use brain_session_status to list active facts.", fact_id
        )),
    }
}

fn tool_session_clear(id: &Value, brain: &mut Brain) -> String {
    brain.session_clear();
    protocol::tool_result(id, "Session memory cleared. Left Brain is now empty. Permanent Right Brain knowledge is unaffected.")
}

// ━━━ Autonomy Engine Tools ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tool_autonomy_status(id: &Value, brain: &Brain) -> String {
    let status = brain.autonomy_status();
    let output = json!({
        "queued_actions": status.queued_actions,
        "policy": status.policy.iter().map(|(a, t)| json!({
            "action_type": a,
            "tier": t
        })).collect::<Vec<_>>(),
        "queue": status.queue.iter().map(|a| json!({
            "id": a.id,
            "action_type": a.action_type,
            "description": a.description,
            "risk_level": a.risk_level,
            "reversible": a.reversible,
            "human_approved": a.human_approved
        })).collect::<Vec<_>>(),
        "recent_history": status.recent_history
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_autonomy_elevate(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let action_type = match get_str(args, "action_type") {
        Some(a) => a,
        None => return protocol::tool_error(id, "Missing required parameter: action_type"),
    };
    let tier = match get_str(args, "tier") {
        Some(t) => t,
        None => return protocol::tool_error(id, "Missing required parameter: tier"),
    };

    match brain.autonomy_set_tier(action_type, tier) {
        Ok(prev) => {
            let msg = format!(
                "Elevated '{}' from {} to {}. Action will now {}.",
                action_type, prev, tier,
                match tier.to_lowercase().as_str() {
                    "observe" => "be read-only (no execution)",
                    "suggest" => "require human approval",
                    "act" => "auto-execute when safe",
                    "create" => "execute with full autonomy",
                    _ => "follow new tier rules",
                }
            );
            protocol::tool_result(id, &msg)
        }
        Err(e) => protocol::tool_error(id, &e),
    }
}

fn tool_autonomy_execute(id: &Value, brain: &mut Brain) -> String {
    match brain.autonomy_execute_next() {
        Some(result) => {
            let output = match &result {
                brain::autonomy::ExecutionResult::Success { action_id, description } => json!({
                    "status": "success",
                    "action_id": action_id,
                    "description": description
                }),
                brain::autonomy::ExecutionResult::Failed { action_id, reason } => json!({
                    "status": "failed",
                    "action_id": action_id,
                    "reason": reason
                }),
                brain::autonomy::ExecutionResult::Blocked { action_id, reason } => json!({
                    "status": "blocked",
                    "action_id": action_id,
                    "reason": reason
                }),
            };
            protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        None => protocol::tool_result(id, "No actions in queue."),
    }
}

fn tool_autonomy_policy(id: &Value, brain: &Brain) -> String {
    let policy = brain.autonomy_policy();
    let output = json!({
        "execution_policy": policy.iter().map(|(a, t)| json!({
            "action_type": a,
            "tier": t
        })).collect::<Vec<_>>(),
        "tier_legend": {
            "Observe": "Read-only, always allowed",
            "Suggest": "Requires human approval (DEFAULT)",
            "Act": "Auto-executes when safe",
            "Create": "Full autonomy including structural changes"
        }
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

// ━━━ Cross-Modal Search Tool ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tool_cross_modal_search(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let query = match get_str(args, "query") {
        Some(q) => q,
        None => return protocol::tool_error(id, "Missing required parameter: query"),
    };
    let limit = get_u64(args, "limit", 10) as usize;

    let results = brain.cross_modal_search(query, limit);
    if results.is_empty() {
        return protocol::tool_result(id, &format!(
            "No media nodes found matching '{}'. The brain may not have ingested any media yet, or similarity is below 0.70 threshold.", query
        ));
    }

    let output = json!({
        "query": query,
        "matches": results.iter().map(|(seed, label, media_type, sim)| json!({
            "seed": seed,
            "label": label,
            "media_type": media_type,
            "similarity": format!("{:.4}", sim)
        })).collect::<Vec<_>>(),
        "count": results.len()
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}


// ━━━ Curiosity Engine Tools ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tool_curiosity_status(id: &Value, brain: &Brain) -> String {
    let status = brain.curiosity_status();
    let output = serde_json::to_string_pretty(&status)
        .unwrap_or_else(|_| "{}".to_string());
    protocol::tool_result(id, &output)
}

fn tool_curiosity_trigger(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let query = match get_str(args, "query") {
        Some(q) => q,
        None => return protocol::tool_error(id, "Missing required parameter: query"),
    };
    let domain = get_str(args, "domain").unwrap_or("general");
    brain.curiosity_trigger(query, domain);
    let status = brain.curiosity_status();
    let output = serde_json::to_string_pretty(&status)
        .unwrap_or_else(|_| "{}".to_string());
    protocol::tool_result(id, &format!(
        "Curiosity triggered for '{}' in domain '{}'.\n{}",
        query, domain, output
    ))
}

fn tool_wonder_queue(id: &Value, args: &Value, brain: &Brain) -> String {
    let count = get_u64(args, "count", 10) as usize;
    let wonders = brain.curiosity_top_wonders(count);
    if wonders.is_empty() {
        return protocol::tool_result(id, "Wonder queue is empty — no knowledge gaps detected yet.");
    }
    let items: Vec<Value> = wonders.iter().map(|w| json!({
        "id": w.id,
        "priority": w.priority_score,
        "domain": w.domain,
        "suggested_query": w.suggested_query,
        "gap_type": format!("{:?}", w.gap),
    })).collect();
    let output = json!({
        "pending_wonders": items.len(),
        "wonders": items
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_curiosity_configure(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let mut actions = Vec::new();
    if let Some(enabled) = args.get("enabled").and_then(|v| v.as_bool()) {
        brain.curiosity_set_enabled(enabled);
        actions.push(format!("Curiosity {}", if enabled { "enabled" } else { "disabled" }));
    }
    if let Some(auto_resolve) = args.get("auto_resolve").and_then(|v| v.as_bool()) {
        brain.curiosity_set_auto_resolve(auto_resolve);
        actions.push(format!(
            "Auto-resolve {}. {}",
            if auto_resolve { "enabled" } else { "disabled" },
            if auto_resolve {
                "Lu will now automatically research and commit knowledge when recall hits a sparse result."
            } else {
                "Wonder resolution is now manual only."
            }
        ));
    }
    if let (Some(domain), Some(weight)) = (
        get_str(args, "domain"),
        args.get("weight").and_then(|v| v.as_f64()),
    ) {
        brain.curiosity_set_interest(domain, weight as f32);
        actions.push(format!("Set interest for '{}' to {:.2}", domain, weight));
    }
    if actions.is_empty() {
        return protocol::tool_error(id, "No configuration parameters provided. Use 'enabled', 'auto_resolve', 'domain'+'weight'.");
    }
    let _ = brain.save();
    protocol::tool_result(id, &actions.join("\n"))
}

fn tool_auto_resolve_wonder(id: &Value, brain: &mut Brain) -> String {
    match brain.auto_resolve_wonder() {
        None => protocol::tool_result(id, "Wonder queue is empty — no gaps to resolve."),
        Some(result) => {
            let output = serde_json::json!({
                "wonder_id": result.wonder_id,
                "query_used": result.query_used,
                "pages_visited": result.pages_visited,
                "facts_proposed": result.facts_proposed,
                "proposals_approved": result.proposals_approved,
                "gap_filled": result.gap_filled,
                "summary": result.summary,
            });
            let _ = brain.save();
            protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
        }
    }
}


// ━━━ Variant Generation Tools ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tool_generate_variants(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let query = match get_str(args, "query") {
        Some(q) => q,
        None => return protocol::tool_error(id, "Missing required parameter: query"),
    };
    let count = get_u64(args, "count", 5) as usize;
    let status = brain.variants_generate(query, count);
    let output = serde_json::to_string_pretty(&status)
        .unwrap_or_else(|_| "{}".to_string());
    protocol::tool_result(id, &format!(
        "Generated {} variants for '{}'. Use brain_variant_status to see scores, brain_variant_promote to promote winners.\n{}",
        status.variant_count, query, output
    ))
}

fn tool_variant_status(id: &Value, brain: &Brain) -> String {
    match brain.variants_status() {
        Some(status) => {
            let output = serde_json::to_string_pretty(&status)
                .unwrap_or_else(|_| "{}".to_string());
            protocol::tool_result(id, &output)
        }
        None => protocol::tool_result(id, "No active variant arena. Use brain_generate_variants first."),
    }
}

fn tool_variant_promote(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let count = get_u64(args, "count", 2) as usize;
    match brain.variants_promote(count) {
        Some(report) => {
            let output = serde_json::to_string_pretty(&report)
                .unwrap_or_else(|_| "{}".to_string());
            protocol::tool_result(id, &format!(
                "Promoted {} variants, archived {}.\n{}",
                report.promoted.len(), report.archived.len(), output
            ))
        }
        None => protocol::tool_result(id, "No active variant arena. Use brain_generate_variants first."),
    }
}


// ━━━ Knowledge Health Tools ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tool_health_report(id: &Value, brain: &Brain) -> String {
    let health = brain.health_report();
    let output = serde_json::to_string_pretty(&health)
        .unwrap_or_else(|_| "{}".to_string());
    protocol::tool_result(id, &output)
}

fn tool_synthesis_pass(id: &Value, brain: &Brain) -> String {
    let report = brain.synthesis_pass();
    if report.proposals.is_empty() {
        return protocol::tool_result(id, &format!(
            "Synthesis pass scanned {} triangles. No missing bonds found — the graph is well-connected in the scanned region.",
            report.triangles_scanned
        ));
    }
    let output = serde_json::to_string_pretty(&report)
        .unwrap_or_else(|_| "{}".to_string());
    protocol::tool_result(id, &format!(
        "Synthesis pass: scanned {} triangles, found {} missing bonds.\n{}",
        report.triangles_scanned, report.missing_bonds_found, output
    ))
}


// ━━━ Invention Engine Tools ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tool_invention_status(id: &Value, brain: &Brain) -> String {
    let status = brain.invention_status();
    let output = serde_json::to_string_pretty(&status)
        .unwrap_or_else(|_| "{}".to_string());
    protocol::tool_result(id, &output)
}


// ━━━ Self-Improvement Tools ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tool_self_improvement_status(id: &Value, brain: &Brain) -> String {
    let status = brain.self_improvement_status();
    let output = serde_json::to_string_pretty(&status)
        .unwrap_or_else(|_| "{}".to_string());
    protocol::tool_result(id, &output)
}

fn tool_meta_reflect(id: &Value, brain: &mut Brain) -> String {
    let reflections = brain.meta_reflect();
    if reflections.is_empty() {
        return protocol::tool_result(id, "Meta-reflection found no patterns to improve. More query data needed.");
    }
    let output = serde_json::to_string_pretty(&reflections)
        .unwrap_or_else(|_| "[]".to_string());
    protocol::tool_result(id, &format!(
        "Meta-reflection produced {} proposals:\n{}", reflections.len(), output
    ))
}


// ━━━ Thinking Simulation Tool ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tool_think_simulate(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let goal = match get_str(args, "goal") {
        Some(g) => g,
        None => return protocol::tool_error(id, "Missing required parameter: goal"),
    };
    match brain.think_simulate(goal) {
        Ok(summary) => {
            let output = serde_json::to_string_pretty(&summary)
                .unwrap_or_else(|_| "{}".to_string());
            protocol::tool_result(id, &format!(
                "Simulation complete (dry-run, no changes made):\n{}", output
            ))
        }
        Err(e) => protocol::tool_error(id, &e),
    }
}

// ━━━ Tool Integration Handlers ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tool_tool_status(id: &Value, brain: &Brain) -> String {
    let status = brain.tool_status();
    let output = serde_json::to_string_pretty(&status)
        .unwrap_or_else(|_| "{}".to_string());
    protocol::tool_result(id, &output)
}

fn tool_tool_detect(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let query = match get_str(args, "query") {
        Some(q) => q,
        None => return protocol::tool_error(id, "Missing required parameter: query"),
    };
    match brain.tool_detect(query) {
        Some((req_id, intent)) => {
            let output = json!({
                "detected": true,
                "request_id": req_id,
                "tool_type": format!("{}", intent.tool_type),
                "confidence": intent.confidence,
                "explanation": intent.explanation,
                "parameters": intent.parameters,
            });
            protocol::tool_result(id, &serde_json::to_string_pretty(&output)
                .unwrap_or_else(|_| "{}".to_string()))
        }
        None => protocol::tool_result(id, "No tool intent detected in query."),
    }
}

fn tool_tool_approve(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let req_id = match get_str(args, "id") {
        Some(i) => i,
        None => return protocol::tool_error(id, "Missing required parameter: id"),
    };
    match brain.tool_approve(req_id) {
        Ok(()) => protocol::tool_result(id, &format!("Tool request {} approved.", req_id)),
        Err(e) => protocol::tool_error(id, &e),
    }
}

fn tool_tool_reject(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let req_id = match get_str(args, "id") {
        Some(i) => i,
        None => return protocol::tool_error(id, "Missing required parameter: id"),
    };
    let reason = get_str(args, "reason").unwrap_or("Rejected by human");
    match brain.tool_reject(req_id, reason) {
        Ok(()) => protocol::tool_result(id, &format!("Tool request {} rejected: {}", req_id, reason)),
        Err(e) => protocol::tool_error(id, &e),
    }
}

fn tool_tool_pending(id: &Value, brain: &Brain) -> String {
    let pending = brain.tool_pending();
    if pending.is_empty() {
        return protocol::tool_result(id, "No tool requests awaiting approval.");
    }
    let output = serde_json::to_string_pretty(&pending)
        .unwrap_or_else(|_| "[]".to_string());
    protocol::tool_result(id, &format!("{} requests awaiting approval:\n{}", pending.len(), output))
}

fn tool_tool_ready(id: &Value, brain: &Brain) -> String {
    let ready = brain.tool_ready();
    if ready.is_empty() {
        return protocol::tool_result(id, "No tool requests ready to execute.");
    }
    let output = serde_json::to_string_pretty(&ready)
        .unwrap_or_else(|_| "[]".to_string());
    protocol::tool_result(id, &format!("{} requests ready to execute:\n{}", ready.len(), output))
}

// ━━━ Orchestrator Handler ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tool_orchestrate(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let input = match get_str(args, "input") {
        Some(i) => i,
        None => return protocol::tool_error(id, "Missing required parameter: input"),
    };
    let result = brain.orchestrate(input);

    // Build clean output - hide internals, show conversational response
    let mut output = result.response.clone();

    // Add pending action hint if any
    if let Some(pending) = &result.pending_action {
        output = format!("{}\n\n[{}]", output, pending);
    }

    // Add scar warnings if any
    if !result.scar_warnings.is_empty() {
        output = format!("{}\n\n⚠ Scar warnings: {}", output,
            result.scar_warnings.join(", "));
    }

    // Persist the exchange so conversation_history survives restarts.
    {
        let session_id = brain.ensure_active_session();
        let _ = brain.add_conversation_message(
            &session_id,
            brain::conversations::MessageRole::User,
            input,
            vec![],
            vec![],
        );
        let _ = brain.add_conversation_message(
            &session_id,
            brain::conversations::MessageRole::Assistant,
            &result.response,
            vec![],
            vec![],
        );
    }

    protocol::tool_result(id, &output)
}


// ====== Web Research Handlers ======

fn tool_web_research(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let query = match get_str(args, "query") {
        Some(q) => q,
        None => return protocol::tool_error(id, "Missing 'query' parameter"),
    };
    let auto_ingest = args.get("ingest").and_then(|v| v.as_bool()).unwrap_or(true);

    let result = brain.web_research(query);

    let mut output = format!("=== Web Research: {} ===\n{}\n", query, result.summary);
    output += &format!("Time: {}ms | Gap filled: {}\n\n", result.total_time_ms, result.gap_filled);

    // Pages visited
    if !result.pages_visited.is_empty() {
        output += "Pages visited:\n";
        for page in &result.pages_visited {
            output += &format!("  [{}] {} (relevance: {:.0}%, {} facts, depth {})\n",
                page.title, page.url, page.relevance_score * 100.0,
                page.paragraphs_extracted, page.depth);
        }
        output += "\n";
    }

    // Top knowledge extracted
    if !result.knowledge_extracted.is_empty() {
        output += &format!("Knowledge extracted ({} facts):\n", result.knowledge_extracted.len());
        for (i, fact) in result.knowledge_extracted.iter().take(10).enumerate() {
            output += &format!("  {}. [conf {:.0}%] {}\n", i + 1, fact.confidence * 100.0, fact.content);
        }
        if result.knowledge_extracted.len() > 10 {
            output += &format!("  ... and {} more\n", result.knowledge_extracted.len() - 10);
        }
        output += "\n";
    }

    // Images found
    if !result.images_found.is_empty() {
        output += &format!("Images found ({}):\n", result.images_found.len());
        for img in result.images_found.iter().take(5) {
            output += &format!("  [img] {} — {}\n", img.description, img.image_url);
        }
        output += "\n";
    }

    // Auto-ingest
    if auto_ingest && !result.knowledge_extracted.is_empty() {
        let count = brain.web_ingest_research(&result);
        output += &format!("Ingested: {} facts proposed for learning.\n", count);
    }

    protocol::tool_result(id, &output)
}

fn tool_web_browse(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let url = match get_str(args, "url") {
        Some(u) => u,
        None => return protocol::tool_error(id, "Missing 'url' parameter"),
    };
    let query = get_str(args, "query").unwrap_or(url);
    let auto_ingest = args.get("ingest").and_then(|v| v.as_bool()).unwrap_or(true);

    let result = brain.web_research_url(url, query);

    let mut output = format!("=== Browsing: {} ===\n{}\n", url, result.summary);
    output += &format!("Time: {}ms | Gap filled: {}\n\n", result.total_time_ms, result.gap_filled);

    for page in &result.pages_visited {
        output += &format!("  [{}] {} ({} facts)\n", page.title, page.url, page.paragraphs_extracted);
    }

    if !result.knowledge_extracted.is_empty() {
        output += &format!("\nKnowledge ({} facts):\n", result.knowledge_extracted.len());
        for (i, fact) in result.knowledge_extracted.iter().take(10).enumerate() {
            output += &format!("  {}. {}\n", i + 1, fact.content);
        }
    }

    if !result.images_found.is_empty() {
        output += &format!("\nImages ({}):\n", result.images_found.len());
        for img in result.images_found.iter().take(5) {
            output += &format!("  [img] {} — {}\n", img.description, img.image_url);
        }
    }

    if auto_ingest && !result.knowledge_extracted.is_empty() {
        let count = brain.web_ingest_research(&result);
        output += &format!("\nIngested: {} facts proposed for learning.\n", count);
    }

    protocol::tool_result(id, &output)
}

fn tool_approve_all(id: &Value, brain: &mut Brain) -> String {
    let pending_count = brain.pending_proposal_ids().len();
    if pending_count == 0 {
        return protocol::tool_result(id, "No pending proposals to approve.");
    }
    let results = brain.approve_all_pending();
    let _ = brain.save();
    let output = format!(
        "Approved {} of {} pending proposals. {} new nodes created.",
        results.len(), pending_count, results.len()
    );
    protocol::tool_result(id, &output)
}

fn tool_web_idle_status(id: &Value, brain: &mut Brain) -> String {
    let stats = brain.web_idle_stats();
    let output = serde_json::to_string_pretty(&stats).unwrap_or_default();
    protocol::tool_result(id, &output)
}

fn tool_web_idle_toggle(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let enabled = args.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    brain.web_set_idle_enabled(enabled);
    let state = if enabled { "enabled" } else { "disabled" };
    protocol::tool_result(id, &format!("Idle exploration {}.", state))
}

fn tool_cleanup_glossary(id: &Value, brain: &mut Brain) -> String {
    let report = brain.cleanup_glossary_nodes();
    let _ = brain.save();

    let output = json!({
        "candidates_found": report.candidates_found,
        "nodes_revoked": report.nodes_revoked,
        "details": report.details.iter().map(|(label, reason)| {
            json!({ "label": label, "reason": reason })
        }).collect::<Vec<_>>()
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_brain_cleanup(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let execute = args.get("execute").and_then(|v| v.as_bool()).unwrap_or(false);
    let min_label_length = get_u64(args, "min_label_length", 10) as usize;

    let report = brain.brain_cleanup(execute, min_label_length);
    if execute {
        let _ = brain.save();
    }

    let output = json!({
        "dry_run": report.dry_run,
        "categories": {
            "page_refs": report.page_refs,
            "textbook_filler": report.textbook_filler,
            "formatting_artifacts": report.formatting_artifacts,
            "too_short": report.too_short,
            "content_junk": report.content_junk
        },
        "total_flagged": report.total_flagged,
        "revoked": report.revoked,
        "sample": report.details.iter().take(200).map(|(label, cat)| {
            json!({ "label": label, "category": cat })
        }).collect::<Vec<_>>()
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}


// ====== Computer Use Tool Handlers ======

fn tool_code_generate(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let description = match get_str(args, "description") {
        Some(d) => d,
        None => return protocol::tool_error(id, "Missing required parameter: description"),
    };
    let code = match get_str(args, "code") {
        Some(c) => c,
        None => return protocol::tool_error(id, "Missing required parameter: code"),
    };
    let language = get_str(args, "language").unwrap_or("python");

    let block = brain.code_generate(description, language, code);

    let output = json!({
        "language": block.language,
        "code": block.code,
        "description": block.description,
        "auto_execute": block.auto_execute,
        "sandbox_dir": brain.sandbox_dir().to_string_lossy(),
    });
    protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn tool_code_execute(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let language = match get_str(args, "language") {
        Some(l) => l,
        None => return protocol::tool_error(id, "Missing required parameter: language"),
    };
    let code_str = match get_str(args, "code") {
        Some(c) => c,
        None => return protocol::tool_error(id, "Missing required parameter: code"),
    };
    let description = get_str(args, "description").unwrap_or("code execution");

    let block = brain::computer_use::CodeBlock {
        language: language.to_string(),
        code: code_str.to_string(),
        description: description.to_string(),
        auto_execute: true,
    };

    match brain.code_execute(&block) {
        Ok(result) => {
            let output = json!({
                "stdout": result.stdout,
                "stderr": result.stderr,
                "exit_code": result.exit_code,
                "duration_ms": result.duration_ms,
                "files_created": result.files_created,
                "timed_out": result.timed_out,
            });
            protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        Err(e) => protocol::tool_error(id, &format!("Execution failed: {}", e)),
    }
}

fn tool_file_read(id: &Value, args: &Value, brain: &Brain) -> String {
    let filename = match get_str(args, "filename") {
        Some(f) => f,
        None => return protocol::tool_error(id, "Missing required parameter: filename"),
    };

    match brain.sandbox_file_read(filename) {
        Ok(content) => {
            let output = json!({
                "filename": filename,
                "content": content,
                "size_bytes": content.len(),
            });
            protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        Err(e) => protocol::tool_error(id, &format!("Read failed: {}", e)),
    }
}

fn tool_file_write(id: &Value, args: &Value, brain: &Brain) -> String {
    let filename = match get_str(args, "filename") {
        Some(f) => f,
        None => return protocol::tool_error(id, "Missing required parameter: filename"),
    };
    let content = match get_str(args, "content") {
        Some(c) => c,
        None => return protocol::tool_error(id, "Missing required parameter: content"),
    };

    match brain.sandbox_file_write(filename, content) {
        Ok(path) => {
            let output = json!({
                "filename": filename,
                "path": path,
                "size_bytes": content.len(),
            });
            protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        Err(e) => protocol::tool_error(id, &format!("Write failed: {}", e)),
    }
}

fn tool_file_list(id: &Value, args: &Value, brain: &Brain) -> String {
    let subdir = get_str(args, "subdir");

    match brain.sandbox_file_list(subdir) {
        Ok(entries) => {
            let output = json!({
                "sandbox_dir": brain.sandbox_dir().to_string_lossy(),
                "entries": entries.iter().map(|e| {
                    json!({
                        "name": e.name,
                        "is_dir": e.is_dir,
                        "size_bytes": e.size_bytes,
                    })
                }).collect::<Vec<_>>(),
                "total": entries.len(),
            });
            protocol::tool_result(id, &serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        Err(e) => protocol::tool_error(id, &format!("List failed: {}", e)),
    }
}


// ─── Think from Wonder ──────────────────────────────────

fn tool_think_from_wonder(id: &Value, args: &Value, brain: &mut Brain) -> String {
    let config = coordinator::ThinkConfig {
        max_iterations: get_u64(args, "max_iterations", 5).min(20) as u32,
        coherence_threshold: args.get("coherence_threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.6) as f32,
        dry_run: args.get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        auto_approve_safe: args.get("auto_approve_safe")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    };

    match brain.think_from_wonder(config) {
        Ok(state) => protocol::tool_result(id, &serde_json::to_string_pretty(&state).unwrap_or_default()),
        Err(e) => protocol::tool_error(id, &e),
    }
}
