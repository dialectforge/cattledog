// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC

import { Route, Switch } from "wouter";
import { Sidebar } from "./components/Sidebar";
import { Chat } from "./pages/Chat";
import { Graph } from "./pages/Graph";
import { Training } from "./pages/Training";
import { Thinking } from "./pages/Thinking";
import { Settings } from "./pages/Settings";

export default function App() {
  return (
    <div className="flex h-screen w-screen bg-gray-900 text-gray-100 overflow-hidden">
      <Sidebar />
      <main className="flex-1 overflow-hidden">
        <Switch>
          <Route path="/"         component={Chat}     />
          <Route path="/graph"    component={Graph}    />
          <Route path="/training" component={Training} />
          <Route path="/thinking" component={Thinking} />
          <Route path="/settings" component={Settings} />
        </Switch>
      </main>
    </div>
  );
}
