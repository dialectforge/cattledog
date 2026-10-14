// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC

import { useEffect, useState } from "react";
import { api, type FlagItem, type BrainStatus } from "../api";

const TIER_ORDER = ["Supervised", "Assisted", "Collaborative", "Autonomous"];

const FLAG_DESCRIPTIONS: Record<string, string> = {
  arena_mode:              "Response variant competition — pick the best of N drafts",
  web_research:            "Allow Lu to research topics via the web",
  code_execution:          "Enable sandboxed code execution in responses",
  rumination:              "Background latent-bond discovery during idle time",
  idle_research:           "Auto-research curiosity items when idle",
  contradiction_detection: "Flag contradictory knowledge during rumination",
  multi_agent_thinking:    "Use RecallAgent → SynthesisAgent → CriticAgent pipeline",
  daemon_mode:             "Background daemon scheduling (manual tick required)",
  style_adaptation:        "Adapt response style to conversation history",
  source_attribution:      "Attribute knowledge sources in responses",
};

export function Settings() {
  const [flags, setFlags]     = useState<FlagItem[]>([]);
  const [status, setStatus]   = useState<BrainStatus | null>(null);
  const [tier, setTier]       = useState("Supervised");
  const [savePath, setSavePath] = useState("");
  const [msg, setMsg]         = useState("");
  const [loading, setLoading] = useState(false);

  async function refresh() {
    try {
      const [f, s] = await Promise.all([api.flagsList(), api.status()]);
      setFlags(f);
      setStatus(s);
      setTier(s.autonomy_tier.charAt(0).toUpperCase() + s.autonomy_tier.slice(1));
    } catch (e) {
      setMsg(`Error: ${e}`);
    }
  }

  useEffect(() => { refresh(); }, []);

  async function toggleFlag(name: string, current: boolean) {
    try {
      await api.flagSet(name, !current);
      setFlags((prev) =>
        prev.map((f) => (f.name === name ? { ...f, enabled: !current } : f))
      );
    } catch (e) {
      setMsg(`Failed to toggle ${name}: ${e}`);
    }
  }

  async function applyConfig() {
    setLoading(true);
    try {
      const result = await api.configure(tier, savePath || undefined);
      setMsg(result);
      await refresh();
    } catch (e) {
      setMsg(`Configure error: ${e}`);
    } finally {
      setLoading(false);
    }
  }

  async function save() {
    try {
      const result = await api.save();
      setMsg(result);
    } catch (e) {
      setMsg(`Save error: ${e}`);
    }
  }

  return (
    <div className="flex flex-col h-full overflow-y-auto">
      <header className="flex items-center justify-between px-4 py-3 border-b border-gray-800 bg-gray-900 shrink-0">
        <span className="text-indigo-400 font-semibold">Settings</span>
        <button
          onClick={refresh}
          className="text-xs px-3 py-1 rounded bg-gray-700 hover:bg-gray-600 text-gray-200 transition-colors"
        >
          Refresh
        </button>
      </header>

      <div className="p-4 space-y-5 max-w-2xl">
        {/* Brain stats summary */}
        {status && (
          <section className="bg-gray-800 rounded-xl p-4 border border-gray-700">
            <h2 className="text-sm font-semibold text-gray-300 mb-3">Brain Status</h2>
            <div className="grid grid-cols-3 gap-x-6 gap-y-2 text-xs">
              {[
                ["Nodes",    `${status.nodes_active.toLocaleString()} / ${status.nodes_total.toLocaleString()}`],
                ["Bonds",    `${status.bonds_active.toLocaleString()} / ${status.bonds_total.toLocaleString()}`],
                ["Queries",  status.total_queries.toLocaleString()],
                ["Episodes", status.episodes.toLocaleString()],
                ["Scars",    status.scars.toLocaleString()],
                ["Proposals", status.pending_proposals.toLocaleString()],
              ].map(([label, value]) => (
                <div key={label} className="flex justify-between">
                  <span className="text-gray-500">{label}</span>
                  <span className="text-gray-200 font-mono">{value}</span>
                </div>
              ))}
            </div>
          </section>
        )}

        {/* Autonomy tier */}
        <section className="bg-gray-800 rounded-xl p-4 border border-gray-700">
          <h2 className="text-sm font-semibold text-gray-300 mb-3">Autonomy Tier</h2>
          <div className="flex gap-2 flex-wrap">
            {TIER_ORDER.map((t) => (
              <button
                key={t}
                onClick={() => setTier(t)}
                className={[
                  "px-3 py-1.5 rounded-lg text-xs font-medium transition-colors",
                  tier === t
                    ? "bg-indigo-600 text-white"
                    : "bg-gray-700 text-gray-400 hover:bg-gray-600 hover:text-gray-200",
                ].join(" ")}
              >
                {t}
              </button>
            ))}
          </div>
          <p className="text-xs text-gray-600 mt-2">
            {tier === "Supervised"    && "All actions require explicit approval."}
            {tier === "Assisted"      && "Learning and thinking enabled; revocation requires approval."}
            {tier === "Collaborative" && "Most operations autonomous; revocation allowed."}
            {tier === "Autonomous"    && "Full autonomy — all operations run without gating."}
          </p>
        </section>

        {/* Storage path */}
        <section className="bg-gray-800 rounded-xl p-4 border border-gray-700">
          <h2 className="text-sm font-semibold text-gray-300 mb-3">Storage</h2>
          <div className="flex gap-2">
            <input
              type="text"
              value={savePath}
              onChange={(e) => setSavePath(e.target.value)}
              placeholder="Leave blank to keep current path…"
              className="flex-1 bg-gray-700 border border-gray-600 rounded px-3 py-1.5 text-xs text-gray-200 placeholder-gray-500 outline-none focus:border-indigo-500 transition-colors"
            />
          </div>
          <div className="flex gap-2 mt-2">
            <button
              onClick={applyConfig}
              disabled={loading}
              className="bg-indigo-600 hover:bg-indigo-500 text-white text-xs px-4 py-1.5 rounded transition-colors disabled:opacity-40"
            >
              {loading ? "Applying…" : "Apply"}
            </button>
            <button
              onClick={save}
              className="bg-gray-700 hover:bg-gray-600 text-gray-200 text-xs px-4 py-1.5 rounded transition-colors"
            >
              Save Brain
            </button>
          </div>
          {msg && (
            <p className="text-xs mt-2 text-indigo-300">{msg}</p>
          )}
        </section>

        {/* Feature flags */}
        <section className="bg-gray-800 rounded-xl p-4 border border-gray-700">
          <h2 className="text-sm font-semibold text-gray-300 mb-3">Feature Flags</h2>
          <div className="space-y-3">
            {flags.map((flag) => (
              <div key={flag.name} className="flex items-center justify-between gap-4">
                <div className="flex-1 min-w-0">
                  <p className="text-xs font-medium text-gray-200">{flag.name}</p>
                  {FLAG_DESCRIPTIONS[flag.name] && (
                    <p className="text-[10px] text-gray-500 mt-0.5">
                      {FLAG_DESCRIPTIONS[flag.name]}
                    </p>
                  )}
                </div>
                <button
                  onClick={() => toggleFlag(flag.name, flag.enabled)}
                  className={[
                    "relative inline-flex w-9 h-5 rounded-full transition-colors shrink-0",
                    flag.enabled ? "bg-indigo-600" : "bg-gray-600",
                  ].join(" ")}
                >
                  <span
                    className={[
                      "absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform",
                      flag.enabled ? "translate-x-4" : "translate-x-0",
                    ].join(" ")}
                  />
                </button>
              </div>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
