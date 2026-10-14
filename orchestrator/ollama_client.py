"""Minimal Ollama HTTP client (Python stdlib only -- no npm, no axios).
Talks to the local Ollama server on the PC. Local model = allowed."""
import json, urllib.request

class Ollama:
    def __init__(self, host='http://127.0.0.1:11434', model='llama3.1:8b'):
        self.host = host
        self.model = model
    def chat(self, messages, tools=None, options=None, timeout=240):
        body = {'model': self.model, 'messages': messages, 'stream': False}
        if tools: body['tools'] = tools
        if options: body['options'] = options
        req = urllib.request.Request(
            self.host + '/api/chat',
            data=json.dumps(body).encode(),
            headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.loads(r.read().decode())
