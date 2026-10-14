// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC

import { useState, useRef, useEffect } from "react";
import { api, type ChatResponse, type WebSearchResult } from "../api";

interface Message {
  id: number;
  role: "user" | "lu";
  text: string;
  timing_ms?: number | null;
  /** Set when message is a web-research result */
  sources?: string[];
}

export function Chat() {
  const [messages, setMessages] = useState<Message[]>([
    {
      id: 0,
      role: "lu",
      text: "Hello. I'm Lu. Ask me anything, or teach me something new.",
    },
  ]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [searching, setSearching] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const nextId = useRef(1);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  async function send() {
    const text = input.trim();
    if (!text || busy || searching) return;

    const userMsg: Message = { id: nextId.current++, role: "user", text };
    setMessages((m) => [...m, userMsg]);
    setInput("");
    setBusy(true);

    try {
      const res: ChatResponse = await api.chat(text);
      setMessages((m) => [
        ...m,
        {
          id: nextId.current++,
          role: "lu",
          text: res.text,
          timing_ms: res.timing_ms,
        },
      ]);
    } catch (err) {
      setMessages((m) => [
        ...m,
        {
          id: nextId.current++,
          role: "lu",
          text: `Error: ${String(err)}`,
        },
      ]);
    } finally {
      setBusy(false);
      inputRef.current?.focus();
    }
  }

  async function webSearch() {
    const query = input.trim();
    if (!query || busy || searching) return;

    const userMsg: Message = {
      id: nextId.current++,
      role: "user",
      text: `🔍 Search: ${query}`,
    };
    setMessages((m) => [...m, userMsg]);
    setInput("");
    setSearching(true);

    try {
      const res: WebSearchResult = await api.webSearch(query);
      const sourceList =
        res.sources.length > 0
          ? `\n\nSources:\n${res.sources.slice(0, 3).map((s) => `• ${s}`).join("\n")}`
          : "";
      setMessages((m) => [
        ...m,
        {
          id: nextId.current++,
          role: "lu",
          text: `${res.summary}${sourceList}`,
          timing_ms: res.elapsed_secs ? res.elapsed_secs * 1000 : null,
          sources: res.sources,
        },
      ]);
    } catch (err) {
      setMessages((m) => [
        ...m,
        {
          id: nextId.current++,
          role: "lu",
          text: `Web search failed: ${String(err)}`,
        },
      ]);
    } finally {
      setSearching(false);
      inputRef.current?.focus();
    }
  }

  function handleKey(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  const isbusy = busy || searching;

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <header className="flex items-center gap-2 px-4 py-3 border-b border-gray-800 bg-gray-900">
        <span className="text-indigo-400 font-semibold">Chat</span>
        <span className="text-gray-600 text-xs">— Lu Brain v2</span>
      </header>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-4 py-4 space-y-4">
        {messages.map((msg) => (
          <div
            key={msg.id}
            className={`flex animate-fade-in ${msg.role === "user" ? "justify-end" : "justify-start"}`}
          >
            {msg.role === "lu" && (
              <div className="w-7 h-7 rounded-full bg-indigo-700 flex items-center justify-center text-xs font-bold text-white mr-2 mt-1 shrink-0">
                L
              </div>
            )}
            <div
              className={[
                "max-w-2xl rounded-xl px-4 py-2.5 text-sm leading-relaxed",
                msg.role === "user"
                  ? "bg-indigo-600 text-white rounded-br-sm"
                  : "bg-gray-800 text-gray-100 rounded-bl-sm",
              ].join(" ")}
            >
              <p className="whitespace-pre-wrap">{msg.text}</p>
              {msg.timing_ms != null && (
                <p className="text-xs mt-1 opacity-40">{msg.timing_ms}ms</p>
              )}
            </div>
          </div>
        ))}

        {(busy || searching) && (
          <div className="flex justify-start animate-fade-in">
            <div className="w-7 h-7 rounded-full bg-indigo-700 flex items-center justify-center text-xs font-bold text-white mr-2 mt-1 shrink-0">
              L
            </div>
            <div className="bg-gray-800 rounded-xl rounded-bl-sm px-4 py-3 flex items-center gap-1.5">
              {searching && (
                <span className="text-xs text-gray-400 mr-1">searching…</span>
              )}
              <span className="thinking-dot" />
              <span className="thinking-dot" />
              <span className="thinking-dot" />
            </div>
          </div>
        )}

        <div ref={bottomRef} />
      </div>

      {/* Input */}
      <div className="border-t border-gray-800 bg-gray-900 px-4 py-3">
        <div className="flex items-end gap-2 bg-gray-800 rounded-xl border border-gray-700 focus-within:border-indigo-500 transition-colors px-3 py-2">
          <textarea
            ref={inputRef}
            rows={1}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKey}
            placeholder="Message Lu… (Enter to send, Shift+Enter for newline)"
            className="flex-1 bg-transparent resize-none outline-none text-sm text-gray-100 placeholder-gray-500 max-h-40"
            style={{ minHeight: "1.5rem" }}
          />
          {/* Web search button */}
          <button
            onClick={webSearch}
            disabled={isbusy || !input.trim()}
            title="Search the web (Grokopedia / DuckDuckGo)"
            className="shrink-0 w-7 h-7 rounded-lg bg-gray-700 flex items-center justify-center text-gray-300 disabled:opacity-40 hover:bg-gray-600 transition-colors"
          >
            🔍
          </button>
          {/* Send button */}
          <button
            onClick={send}
            disabled={isbusy || !input.trim()}
            className="shrink-0 w-7 h-7 rounded-lg bg-indigo-600 flex items-center justify-center text-white disabled:opacity-40 hover:bg-indigo-500 transition-colors"
          >
            ↑
          </button>
        </div>
        <p className="text-xs text-gray-600 mt-1 px-1">
          🔍 search the web · ↑ chat with Lu
        </p>
      </div>
    </div>
  );
}
