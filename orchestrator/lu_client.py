"""Cross-platform MCP stdio client for a local Lu (lu_mcp_v2.exe).
Thread-based reader so it works under both Windows Python and WSL Python.
Cattle Dog is a CLIENT of Lu; this only ever talks to a sandbox brain
Cattle Dog owns -- never the canonical Hetzner training brain."""
import subprocess, json, threading, queue, time

class LuClient:
    def __init__(self, exe, brain, stderr_log=None):
        self.exe, self.brain = exe, brain
        err = open(stderr_log, 'ab') if stderr_log else subprocess.DEVNULL
        self.p = subprocess.Popen([exe, brain], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=err, text=True, bufsize=1)
        self._q = queue.Queue()
        self._id = 0
        self._t = threading.Thread(target=self._reader, daemon=True)
        self._t.start()
    def _reader(self):
        for line in self.p.stdout:
            self._q.put(line)
    def _send(self, o):
        self.p.stdin.write(json.dumps(o) + '\n'); self.p.stdin.flush()
    def _await(self, mid, timeout):
        t = time.time()
        while time.time() - t < timeout:
            try:
                line = self._q.get(timeout=timeout)
            except queue.Empty:
                break
            line = line.strip()
            if not line.startswith('{'):
                continue
            try:
                o = json.loads(line)
            except Exception:
                continue
            if o.get('id') == mid:
                return o
        raise TimeoutError('rpc id=%s' % mid)
    def _rpc(self, method, params, timeout=60):
        self._id += 1; mid = self._id
        self._send({'jsonrpc': '2.0', 'id': mid, 'method': method, 'params': params})
        return self._await(mid, timeout)
    def initialize(self):
        r = self._rpc('initialize', {'protocolVersion': '2024-11-05',
            'capabilities': {}, 'clientInfo': {'name': 'cattledog', 'version': '0.1'}})
        self._send({'jsonrpc': '2.0', 'method': 'notifications/initialized'})
        return r
    def call(self, name, args=None, timeout=120):
        o = self._rpc('tools/call', {'name': name, 'arguments': args or {}}, timeout)
        res = o.get('result', {})
        return ''.join(c.get('text', '') for c in res.get('content', []) if isinstance(c, dict))
    def wait_ready(self, tries=60, delay=2.0):
        for _ in range(tries):
            s = self.call('lu_status', {})
            if 'still loading' in s.lower():
                time.sleep(delay); continue
            return s
        raise TimeoutError('brain load timeout')
    def close(self):
        try:
            self.p.stdin.close(); self.p.wait(timeout=10)
        except Exception:
            self.p.kill()
