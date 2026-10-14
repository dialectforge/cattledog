// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC

import { useEffect, useRef, useState, useCallback } from "react";
import { api, type GraphData, type GraphNode, type GraphBond } from "../api";

// ── Simple force-directed graph rendered on <canvas> ─────────────────────────

interface SimNode extends GraphNode {
  x: number;
  y: number;
  vx: number;
  vy: number;
}

interface SimBond extends GraphBond {
  source: string;
  target: string;
}

const KU_COLORS: Record<string, string> = {
  Definition:  "#818cf8",
  Mechanism:   "#34d399",
  Composition: "#f59e0b",
  Purpose:     "#60a5fa",
  Contrast:    "#f472b6",
  Causal:      "#fb923c",
  Example:     "#a3e635",
  Procedure:   "#38bdf8",
  Code:        "#e879f9",
  Raw:         "#6b7280",
};

function kuColor(type: string) {
  return KU_COLORS[type] ?? "#6b7280";
}

export function Graph() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animRef = useRef<number>(0);
  const nodesRef = useRef<Map<string, SimNode>>(new Map());
  const bondsRef = useRef<SimBond[]>([]);
  const [data, setData] = useState<GraphData | null>(null);
  const [loading, setLoading] = useState(false);
  const [selectedNode, setSelectedNode] = useState<SimNode | null>(null);
  const [limit, setLimit] = useState(200);
  const dragRef = useRef<{ nodeId: string | null; ox: number; oy: number }>({
    nodeId: null,
    ox: 0,
    oy: 0,
  });
  const offsetRef = useRef({ x: 0, y: 0, scale: 1 });
  const panRef = useRef<{ active: boolean; startX: number; startY: number }>({
    active: false,
    startX: 0,
    startY: 0,
  });

  const loadGraph = useCallback(async () => {
    setLoading(true);
    try {
      const graphResult = await api.graphData(limit);
      setData(graphResult);

      // Initialise simulation nodes with random positions
      const canvas = canvasRef.current;
      const w = canvas?.width ?? 800;
      const h = canvas?.height ?? 600;
      const newNodes = new Map<string, SimNode>();
      for (const n of graphResult.nodes) {
        const existing = nodesRef.current.get(n.id);
        newNodes.set(n.id, existing ?? {
          ...n,
          x: Math.random() * w,
          y: Math.random() * h,
          vx: 0,
          vy: 0,
        });
      }
      nodesRef.current = newNodes;
      bondsRef.current = graphResult.bonds as SimBond[];
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  }, [limit]);

  // ── Force simulation tick ─────────────────────────────────────────────────

  const tick = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const W = canvas.width;
    const H = canvas.height;
    const nodes = Array.from(nodesRef.current.values());

    // Repulsion
    for (let i = 0; i < nodes.length; i++) {
      for (let j = i + 1; j < nodes.length; j++) {
        const a = nodes[i];
        const b = nodes[j];
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const d2 = dx * dx + dy * dy + 1;
        const force = 3000 / d2;
        const fx = force * dx;
        const fy = force * dy;
        a.vx -= fx; a.vy -= fy;
        b.vx += fx; b.vy += fy;
      }
    }

    // Attraction along bonds
    for (const bond of bondsRef.current) {
      const a = nodesRef.current.get(bond.source);
      const b = nodesRef.current.get(bond.target);
      if (!a || !b) continue;
      const dx = b.x - a.x;
      const dy = b.y - a.y;
      const dist = Math.sqrt(dx * dx + dy * dy) + 0.01;
      void dist; // used only for scaling; keep for future spring-length logic
      const strength = bond.weight * 0.015;
      a.vx += dx * strength; a.vy += dy * strength;
      b.vx -= dx * strength; b.vy -= dy * strength;
    }

    // Centering
    for (const n of nodes) {
      n.vx += (W / 2 - n.x) * 0.001;
      n.vy += (H / 2 - n.y) * 0.001;
      n.vx *= 0.82;
      n.vy *= 0.82;
      n.x = Math.max(10, Math.min(W - 10, n.x + n.vx));
      n.y = Math.max(10, Math.min(H - 10, n.y + n.vy));
    }

    // Draw
    const { x: ox, y: oy, scale: sc } = offsetRef.current;
    ctx.clearRect(0, 0, W, H);
    ctx.save();
    ctx.translate(ox, oy);
    ctx.scale(sc, sc);

    // Bonds
    ctx.globalAlpha = 0.25;
    for (const bond of bondsRef.current) {
      const a = nodesRef.current.get(bond.source);
      const b = nodesRef.current.get(bond.target);
      if (!a || !b) continue;
      ctx.strokeStyle = "#4b5563";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      ctx.stroke();
    }

    // Nodes
    ctx.globalAlpha = 1;
    for (const n of nodes) {
      const r = 4 + Math.min(n.activation_count * 0.3, 8);
      const color = kuColor(n.ku_type);
      ctx.beginPath();
      ctx.arc(n.x, n.y, r, 0, Math.PI * 2);
      ctx.fillStyle = color + "cc";
      ctx.fill();
      ctx.strokeStyle = color;
      ctx.lineWidth = 1;
      ctx.stroke();
    }

    // Selected node label
    if (selectedNode) {
      const n = nodesRef.current.get(selectedNode.id);
      if (n) {
        ctx.fillStyle = "#0a0a0f";
        const label = n.label.slice(0, 50);
        const tw = ctx.measureText(label).width;
        ctx.fillRect(n.x + 10, n.y - 16, tw + 8, 20);
        ctx.fillStyle = "#e2e8f0";
        ctx.font = "11px Inter, sans-serif";
        ctx.fillText(label, n.x + 14, n.y - 2);
      }
    }

    ctx.restore();
    animRef.current = requestAnimationFrame(tick);
  }, [selectedNode]);

  useEffect(() => {
    animRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(animRef.current);
  }, [tick]);

  // Resize canvas on mount
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ro = new ResizeObserver(() => {
      canvas.width  = canvas.offsetWidth;
      canvas.height = canvas.offsetHeight;
    });
    ro.observe(canvas);
    return () => ro.disconnect();
  }, []);

  // Pointer events for drag/pan/select
  function pickNode(cx: number, cy: number): SimNode | null {
    const { x: ox, y: oy, scale: sc } = offsetRef.current;
    const wx = (cx - ox) / sc;
    const wy = (cy - oy) / sc;
    for (const n of nodesRef.current.values()) {
      const dx = n.x - wx;
      const dy = n.y - wy;
      if (dx * dx + dy * dy < 100) return n;
    }
    return null;
  }

  function onPointerDown(e: React.PointerEvent<HTMLCanvasElement>) {
    const rect = canvasRef.current!.getBoundingClientRect();
    const cx = e.clientX - rect.left;
    const cy = e.clientY - rect.top;
    const hit = pickNode(cx, cy);
    if (hit) {
      dragRef.current = { nodeId: hit.id, ox: hit.x, oy: hit.y };
      setSelectedNode(hit);
    } else {
      panRef.current = { active: true, startX: cx - offsetRef.current.x, startY: cy - offsetRef.current.y };
    }
  }

  function onPointerMove(e: React.PointerEvent<HTMLCanvasElement>) {
    const rect = canvasRef.current!.getBoundingClientRect();
    const cx = e.clientX - rect.left;
    const cy = e.clientY - rect.top;
    if (dragRef.current.nodeId) {
      const { x: ox, y: oy, scale: sc } = offsetRef.current;
      const n = nodesRef.current.get(dragRef.current.nodeId);
      if (n) { n.x = (cx - ox) / sc; n.y = (cy - oy) / sc; n.vx = 0; n.vy = 0; }
    } else if (panRef.current.active) {
      offsetRef.current.x = cx - panRef.current.startX;
      offsetRef.current.y = cy - panRef.current.startY;
    }
  }

  function onPointerUp() {
    dragRef.current.nodeId = null;
    panRef.current.active = false;
  }

  function onWheel(e: React.WheelEvent<HTMLCanvasElement>) {
    e.preventDefault();
    const factor = e.deltaY > 0 ? 0.9 : 1.1;
    offsetRef.current.scale = Math.max(0.2, Math.min(4, offsetRef.current.scale * factor));
  }

  return (
    <div className="flex flex-col h-full">
      <header className="flex items-center justify-between px-4 py-3 border-b border-gray-800 bg-gray-900 shrink-0">
        <span className="text-indigo-400 font-semibold">Knowledge Graph</span>
        <div className="flex items-center gap-3">
          {data && (
            <span className="text-xs text-gray-500">
              {data.total_nodes.toLocaleString()} nodes · {data.total_bonds.toLocaleString()} bonds
              {data.nodes.length < data.total_nodes && ` · showing ${data.nodes.length}`}
            </span>
          )}
          <select
            value={limit}
            onChange={(e) => setLimit(Number(e.target.value))}
            className="bg-gray-800 border border-gray-700 rounded px-2 py-1 text-xs text-gray-300"
          >
            <option value={100}>100 nodes</option>
            <option value={200}>200 nodes</option>
            <option value={500}>500 nodes</option>
          </select>
          <button
            onClick={loadGraph}
            disabled={loading}
            className="bg-indigo-600 hover:bg-indigo-500 text-white text-xs px-3 py-1 rounded transition-colors disabled:opacity-40"
          >
            {loading ? "Loading…" : "Load"}
          </button>
        </div>
      </header>

      {/* Legend */}
      <div className="flex flex-wrap gap-x-4 gap-y-1 px-4 py-2 border-b border-gray-800 bg-gray-900/50 shrink-0">
        {Object.entries(KU_COLORS).map(([type, color]) => (
          <span key={type} className="flex items-center gap-1 text-xs text-gray-400">
            <span className="w-2 h-2 rounded-full inline-block" style={{ background: color }} />
            {type}
          </span>
        ))}
      </div>

      <div className="flex flex-1 overflow-hidden">
        {/* Canvas */}
        <canvas
          ref={canvasRef}
          className="flex-1 bg-gray-950 cursor-grab active:cursor-grabbing"
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerLeave={onPointerUp}
          onWheel={onWheel}
          style={{ touchAction: "none" }}
        />

        {/* Selected node detail */}
        {selectedNode && (
          <aside className="w-64 border-l border-gray-800 bg-gray-900 p-4 overflow-y-auto shrink-0">
            <div className="flex justify-between items-start mb-3">
              <h3 className="text-sm font-semibold text-indigo-300">Node</h3>
              <button
                onClick={() => setSelectedNode(null)}
                className="text-gray-500 hover:text-gray-300 text-xs"
              >
                ✕
              </button>
            </div>
            <div className="space-y-2 text-xs">
              <div>
                <span className="text-gray-500 block mb-0.5">Type</span>
                <span className="text-gray-200" style={{ color: kuColor(selectedNode.ku_type) }}>
                  {selectedNode.ku_type}
                </span>
              </div>
              <div>
                <span className="text-gray-500 block mb-0.5">Confidence</span>
                <span className="text-gray-200">{selectedNode.confidence}</span>
              </div>
              <div>
                <span className="text-gray-500 block mb-0.5">Activations</span>
                <span className="text-gray-200">{selectedNode.activation_count}</span>
              </div>
              <div>
                <span className="text-gray-500 block mb-0.5">Content</span>
                <p className="text-gray-300 leading-relaxed">{selectedNode.label}</p>
              </div>
              <div>
                <span className="text-gray-500 block mb-0.5">Seed</span>
                <span className="text-gray-500 font-mono text-[10px] break-all">
                  {selectedNode.id.slice(0, 24)}…
                </span>
              </div>
            </div>
          </aside>
        )}
      </div>

      {!data && !loading && (
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <p className="text-gray-600 text-sm">Press "Load" to visualise the knowledge graph</p>
        </div>
      )}
    </div>
  );
}
