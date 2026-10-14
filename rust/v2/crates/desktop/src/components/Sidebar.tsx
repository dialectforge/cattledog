// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC

import { Link, useLocation } from "wouter";

interface NavItem {
  to: string;
  icon: string;
  label: string;
}

const NAV: NavItem[] = [
  { to: "/",         icon: "💬", label: "Chat"     },
  { to: "/graph",    icon: "🧠", label: "Graph"    },
  { to: "/training", icon: "📚", label: "Training" },
  { to: "/thinking", icon: "💡", label: "Thinking" },
  { to: "/settings", icon: "⚙️",  label: "Settings" },
];

export function Sidebar() {
  const [location] = useLocation();

  return (
    <aside className="flex flex-col w-14 bg-gray-950 border-r border-gray-800 py-3 items-center gap-1 shrink-0">
      {/* Lu logo */}
      <div className="w-8 h-8 rounded-full bg-indigo-600 flex items-center justify-center text-white font-bold text-sm mb-3">
        L
      </div>

      {NAV.map(({ to, icon, label }) => {
        const active = to === "/" ? location === "/" : location.startsWith(to);
        return (
          <Link key={to} href={to}>
            <button
              title={label}
              className={[
                "w-10 h-10 rounded-lg flex items-center justify-center text-lg",
                "transition-colors duration-150",
                active
                  ? "bg-indigo-600/30 text-indigo-300"
                  : "text-gray-500 hover:bg-gray-800 hover:text-gray-200",
              ].join(" ")}
            >
              {icon}
            </button>
          </Link>
        );
      })}
    </aside>
  );
}
