// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC

import { useEffect, useState, useRef } from "react";
import { api, type BrainStatus, type ProposalItem, type LearnResult } from "../api";

function StatCard({ label, value, sub }: { label: string; value: string | number; sub?: string }) {
  return (
    <div className="bg-gray-800 rounded-xl p-4 border border-gray-700">
      <p className="text-gray-500 text-xs mb-1">{label}</p>
      <p className="text-2xl font-bold text-indigo-300">{typeof value === "number" ? value.toLocaleString() : value}</p>
      {sub && <p className="text-gray-600 text-xs mt-0.5">{sub}</p>}
    </div>
  );
}

function MetricBar({ label, value }: { label: string; value: number }) {
  const pct = Math.round(value * 100);
  return (
    <div>
      <div className="flex justify-between text-xs text-gray-400 mb-1">
        <span>{label}</span>
        <span>{pct}%</span>
      </div>
      <div className="h-2 bg-gray-700 rounded-full overflow-hidden">
        <div
          className="h-full rounded-full bg-indigo-500 transition-all duration-500"
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}

export function Training() {
  const [status, setStatus] = useState<BrainStatus | null>(null);
  const [proposals, setProposals] = useState<ProposalItem[]>([]);
  const [learnText, setLearnText] = useState("");
  const [confidence, setConfidence] = useState("Terminal");
  const [busy, setBusy] = useState(false);
  const [log, setLog] = useState<string[]>([]);
  const [consolidating, setConsolidating] = useState(false);
  const logRef = useRef<HTMLDivElement>(null);

  function pushLog(msg: string) {
    setLog((l) => [...l.slice(-99), `[${new Date().toLocaleTimeString()}] ${msg}`]);
  }

  async function refresh() {
    try {
      const [s, p] = await Promise.all([api.status(), api.proposalsList()]);
      setStatus(s);
      setProposals(p);
    } catch (e) {
      pushLog(`Error refreshing: ${e}`);
    }
  }

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 5000);
    return () => clearInterval(id);
  }, []);

  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight, behavior: "smooth" });
  }, [log]);

  async function learn() {
    if (!learnText.trim() || busy) return;
    setBusy(true);
    try {
      const res: LearnResult = await api.learn(learnText.trim(), confidence);
      pushLog(`Learned ${res.count} KU(s). Seeds: ${res.seeds.slice(0, 3).join(", ")}${res.seeds.length > 3 ? "…" : ""}`);
      setLearnText("");
      await refresh();
    } catch (e) {
      pushLog(`Learn error: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  async function consolidate() {
    setConsolidating(true);
    try {
      const res = await api.consolidate();
      pushLog(`Consolidation: ${res.episodes_processed} episodes processed, ${res.bonds_strengthened} bonds strengthened`);
      await refresh();
    } catch (e) {
      pushLog(`Consolidation error: ${e}`);
    } finally {
      setConsolidating(false);
    }
  }

  async function save() {
    try {
      const msg = await api.save();
      pushLog(msg);
    } catch (e) {
      pushLog(`Save error: ${e}`);
    }
  }

  async function approveProposal(id: string) {
    try {
      const msg = await api.proposalApprove(id);
      pushLog(msg);
      setProposals((p) => p.filter((x) => x.id !== id));
    } catch (e) {
      pushLog(`Approve error: ${e}`);
    }
  }

  async function rejectProposal(id: string) {
    try {
      const msg = await api.proposalReject(id);
      pushLog(msg);
      setProposals((p) => p.filter((x) => x.id !== id));
    } catch (e) {
      pushLog(`Reject error: ${e}`);
    }
  }

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <header className="flex items-center justify-between px-4 py-3 border-b border-gray-800 bg-gray-900 shrink-0">
        <span className="text-indigo-400 font-semibold">Training</span>
        <div className="flex gap-2">
          <button
            onClick={consolidate}
            disabled={consolidating}
            className="text-xs px-3 py-1 rounded bg-gray-700 hover:bg-gray-600 text-gray-200 disabled:opacity-40 transition-colors"
          >
            {consolidating ? "Consolidating…" : "Consolidate"}
          </button>
          <button
            onClick={save}
            className="text-xs px-3 py-1 rounded bg-gray-700 hover:bg-gray-600 text-gray-200 transition-colors"
          >
            Save
          </button>
          <button
            onClick={refresh}
            className="text-xs px-3 py-1 rounded bg-indigo-600 hover:bg-indigo-500 text-white transition-colors"
          >
            Refresh
          </button>
        </div>
      </header>

      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* Stats grid */}
        {status && (
          <div className="grid grid-cols-4 gap-3">
            <StatCard label="Total Nodes" value={status.nodes_total} sub={`${status.nodes_active} active`} />
            <StatCard label="Total Bonds" value={status.bonds_total} sub={`${status.bonds_active} active`} />
            <StatCard label="Episodes" value={status.episodes} />
            <StatCard label="Total Queries" value={status.total_queries} sub={`${status.scars} scars`} />
          </div>
        )}

        <div className="grid grid-cols-2 gap-4">
          {/* Performance metrics */}
          {status && (
            <div className="bg-gray-800 rounded-xl p-4 border border-gray-700 space-y-3">
              <h3 className="text-sm font-semibold text-gray-300 mb-3">Performance</h3>
              <MetricBar label="Recall Hit Rate" value={status.recall_hit_rate} />
              <MetricBar label="Response Quality" value={status.avg_response_quality} />
              <MetricBar label="Consolidation Effectiveness" value={status.consolidation_effectiveness} />
              <div className="pt-2 border-t border-gray-700">
                <span className="text-xs text-gray-500">Autonomy Tier: </span>
                <span className="text-xs text-indigo-300 font-medium">{status.autonomy_tier}</span>
              </div>
              {status.warnings.length > 0 && (
                <div className="space-y-1">
                  {status.warnings.map((w, i) => (
                    <p key={i} className="text-xs text-amber-400">⚠ {w}</p>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Pending proposals */}
          <div className="bg-gray-800 rounded-xl p-4 border border-gray-700">
            <h3 className="text-sm font-semibold text-gray-300 mb-3">
              Pending Proposals
              {proposals.length > 0 && (
                <span className="ml-2 bg-indigo-600 text-white text-xs rounded-full px-1.5 py-0.5">
                  {proposals.length}
                </span>
              )}
            </h3>
            {proposals.length === 0 ? (
              <p className="text-xs text-gray-600">No pending proposals</p>
            ) : (
              <div className="space-y-2 max-h-48 overflow-y-auto">
                {proposals.map((p) => (
                  <div key={p.id} className="bg-gray-700/50 rounded-lg p-2.5">
                    <div className="flex justify-between items-start gap-2">
                      <div className="flex-1 min-w-0">
                        <span className="text-xs text-indigo-400 font-medium">{p.kind}</span>
                        <p className="text-xs text-gray-300 mt-0.5 leading-snug truncate">{p.summary}</p>
                      </div>
                      <div className="flex gap-1 shrink-0">
                        <button
                          onClick={() => approveProposal(p.id)}
                          className="text-xs px-1.5 py-0.5 rounded bg-green-700/50 hover:bg-green-600/60 text-green-300 transition-colors"
                        >
                          ✓
                        </button>
                        <button
                          onClick={() => rejectProposal(p.id)}
                          className="text-xs px-1.5 py-0.5 rounded bg-red-700/50 hover:bg-red-600/60 text-red-300 transition-colors"
                        >
                          ✕
                        </button>
                      </div>
                    </div>
                    <p className="text-[10px] text-gray-600 mt-1">{new Date(p.created_at).toLocaleString()}</p>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Learn input */}
        <div className="bg-gray-800 rounded-xl p-4 border border-gray-700">
          <h3 className="text-sm font-semibold text-gray-300 mb-3">Teach Lu</h3>
          <textarea
            value={learnText}
            onChange={(e) => setLearnText(e.target.value)}
            placeholder="Enter knowledge to teach Lu…"
            rows={4}
            className="w-full bg-gray-700 border border-gray-600 rounded-lg px-3 py-2 text-sm text-gray-200 placeholder-gray-500 resize-y outline-none focus:border-indigo-500 transition-colors"
          />
          <div className="flex items-center gap-3 mt-2">
            <select
              value={confidence}
              onChange={(e) => setConfidence(e.target.value)}
              className="bg-gray-700 border border-gray-600 rounded px-2 py-1 text-xs text-gray-300"
            >
              <option value="Terminal">Terminal</option>
              <option value="Verified">Verified</option>
              <option value="Unverified">Unverified</option>
              <option value="Inferred">Inferred</option>
            </select>
            <button
              onClick={learn}
              disabled={busy || !learnText.trim()}
              className="bg-indigo-600 hover:bg-indigo-500 text-white text-xs px-4 py-1.5 rounded transition-colors disabled:opacity-40"
            >
              {busy ? "Learning…" : "Teach"}
            </button>
          </div>
        </div>

        {/* Activity log */}
        <div className="bg-gray-800 rounded-xl border border-gray-700">
          <h3 className="text-sm font-semibold text-gray-300 px-4 py-3 border-b border-gray-700">Activity Log</h3>
          <div
            ref={logRef}
            className="h-32 overflow-y-auto px-4 py-3 space-y-1 font-mono"
          >
            {log.length === 0 ? (
              <p className="text-xs text-gray-600">No activity yet</p>
            ) : (
              log.map((line, i) => (
                <p key={i} className="text-xs text-gray-400">{line}</p>
              ))
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
