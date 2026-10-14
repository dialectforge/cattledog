// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC

import { useState } from "react";
import { api, type ObserveData } from "../api";

interface ThinkStep {
  agent: string;
  color: string;
  content: string;
}

function parseThinkSteps(raw: string): ThinkStep[] {
  // Try to split the thinking output into conceptual sections
  const lines = raw.split("\n").filter((l) => l.trim());
  const steps: ThinkStep[] = [];

  const agentPatterns: [RegExp, string, string][] = [
    [/^##?\s*recall|recall agent|knowledge recall/i,      "RecallAgent",    "#818cf8"],
    [/^##?\s*synthes|synthesis agent|synthesizing/i,      "SynthesisAgent", "#34d399"],
    [/^##?\s*critic|critic agent|critique|critical/i,     "CriticAgent",    "#fb923c"],
    [/^##?\s*conclusion|final answer|summary/i,           "Summary",        "#60a5fa"],
  ];

  let currentAgent = "RecallAgent";
  let currentColor = "#818cf8";
  let buffer: string[] = [];

  function flush() {
    if (buffer.length > 0) {
      steps.push({ agent: currentAgent, color: currentColor, content: buffer.join("\n").trim() });
      buffer = [];
    }
  }

  for (const line of lines) {
    let matched = false;
    for (const [pattern, agent, color] of agentPatterns) {
      if (pattern.test(line)) {
        flush();
        currentAgent = agent;
        currentColor = color;
        matched = true;
        break;
      }
    }
    if (!matched) buffer.push(line);
  }
  flush();

  // If parsing found nothing useful, return the whole text as one step
  if (steps.length === 0 && raw.trim()) {
    steps.push({ agent: "Response", color: "#818cf8", content: raw });
  }
  return steps;
}

function AgentBadge({ agent, color }: { agent: string; color: string }) {
  return (
    <span
      className="inline-flex items-center gap-1 text-[10px] font-semibold px-2 py-0.5 rounded-full"
      style={{ background: color + "22", color }}
    >
      <span className="w-1.5 h-1.5 rounded-full" style={{ background: color }} />
      {agent}
    </span>
  );
}

export function Thinking() {
  const [topic, setTopic] = useState("");
  const [steps, setSteps] = useState<ThinkStep[]>([]);
  const [observe, setObserve] = useState<ObserveData | null>(null);
  const [busy, setBusy] = useState(false);
  const [timingMs, setTimingMs] = useState<number | null>(null);

  async function think() {
    if (!topic.trim() || busy) return;
    setBusy(true);
    setSteps([]);
    setTimingMs(null);
    try {
      const [res, obs] = await Promise.all([
        api.think(topic.trim()),
        api.observe(),
      ]);
      setSteps(parseThinkSteps(res.answer));
      setTimingMs(res.timing_ms);
      setObserve(obs);
    } catch (e) {
      setSteps([{ agent: "Error", color: "#f87171", content: String(e) }]);
    } finally {
      setBusy(false);
    }
  }

  async function refreshObserve() {
    try {
      setObserve(await api.observe());
    } catch (_) {}
  }

  return (
    <div className="flex flex-col h-full">
      <header className="flex items-center justify-between px-4 py-3 border-b border-gray-800 bg-gray-900 shrink-0">
        <span className="text-indigo-400 font-semibold">Thinking</span>
        <span className="text-gray-600 text-xs">RecallAgent → SynthesisAgent → CriticAgent</span>
      </header>

      <div className="flex flex-1 overflow-hidden">
        {/* Main panel */}
        <div className="flex flex-col flex-1 overflow-hidden">
          {/* Input */}
          <div className="px-4 py-3 border-b border-gray-800 bg-gray-900/50 shrink-0">
            <div className="flex gap-2">
              <input
                type="text"
                value={topic}
                onChange={(e) => setTopic(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && think()}
                placeholder="Enter a topic for deep thinking…"
                className="flex-1 bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-sm text-gray-200 placeholder-gray-500 outline-none focus:border-indigo-500 transition-colors"
              />
              <button
                onClick={think}
                disabled={busy || !topic.trim()}
                className="bg-indigo-600 hover:bg-indigo-500 text-white text-sm px-4 py-2 rounded-lg transition-colors disabled:opacity-40 shrink-0"
              >
                {busy ? "Thinking…" : "Think"}
              </button>
            </div>
            {timingMs != null && (
              <p className="text-xs text-gray-600 mt-1">Completed in {timingMs.toLocaleString()}ms</p>
            )}
          </div>

          {/* Steps */}
          <div className="flex-1 overflow-y-auto p-4 space-y-3">
            {busy && (
              <div className="flex items-center gap-3 animate-fade-in">
                <div className="w-2 h-2 rounded-full bg-indigo-400" />
                <span className="text-sm text-gray-400">
                  Running multi-agent reasoning pipeline…
                </span>
                <span className="thinking-dot" />
                <span className="thinking-dot" />
                <span className="thinking-dot" />
              </div>
            )}

            {steps.map((step, i) => (
              <div
                key={i}
                className="bg-gray-800 rounded-xl border border-gray-700 p-4 animate-fade-in"
                style={{ borderLeftColor: step.color, borderLeftWidth: 3 }}
              >
                <div className="mb-2">
                  <AgentBadge agent={step.agent} color={step.color} />
                </div>
                <p className="text-sm text-gray-200 leading-relaxed whitespace-pre-wrap">
                  {step.content}
                </p>
              </div>
            ))}

            {steps.length === 0 && !busy && (
              <div className="flex flex-col items-center justify-center h-full gap-3 text-center">
                <div className="text-4xl">💡</div>
                <p className="text-gray-600 text-sm max-w-md">
                  Enter a topic to watch Lu's multi-agent reasoning pipeline work through it —
                  RecallAgent pulls relevant knowledge, SynthesisAgent assembles a response,
                  CriticAgent reviews and refines.
                </p>
              </div>
            )}
          </div>
        </div>

        {/* Right panel — live brain observe */}
        <aside className="w-56 border-l border-gray-800 bg-gray-900 p-4 overflow-y-auto shrink-0">
          <div className="flex justify-between items-center mb-3">
            <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wider">Brain State</h3>
            <button onClick={refreshObserve} className="text-gray-600 hover:text-gray-300 text-xs">↻</button>
          </div>

          {observe ? (
            <div className="space-y-3">
              <div className="space-y-2">
                {[
                  { label: "Active Nodes",   value: observe.active_nodes   },
                  { label: "Active Bonds",   value: observe.active_bonds   },
                  { label: "Episodes",       value: observe.episodes_total  },
                  { label: "Pending Consolidation", value: observe.pending_consolidation },
                  { label: "Curiosity Queue", value: observe.curiosity_queue },
                ].map(({ label, value }) => (
                  <div key={label} className="flex justify-between text-xs">
                    <span className="text-gray-500">{label}</span>
                    <span className="text-indigo-300 font-mono">{value.toLocaleString()}</span>
                  </div>
                ))}
              </div>

              {observe.wonder_items.length > 0 && (
                <div>
                  <p className="text-xs text-gray-500 mb-1.5">Recent Episodes</p>
                  <div className="space-y-1.5">
                    {observe.wonder_items.map((item, i) => (
                      <p key={i} className="text-[10px] text-gray-500 leading-snug border-l-2 border-gray-700 pl-2">
                        {item.length > 80 ? item.slice(0, 80) + "…" : item}
                      </p>
                    ))}
                  </div>
                </div>
              )}
            </div>
          ) : (
            <p className="text-xs text-gray-600">Think on a topic to load brain state</p>
          )}
        </aside>
      </div>
    </div>
  );
}
