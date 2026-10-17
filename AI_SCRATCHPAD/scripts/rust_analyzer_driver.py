#!/usr/bin/env python3
"""Drive rust-analyzer from the command line: rename symbols and move files,
applying the WorkspaceEdits it returns.

  rust_analyzer_driver.py rename <file> <line> <column> <new_name> [...more renames as groups of 4]
  rust_analyzer_driver.py move <old_path> <new_path> [...more pairs]
  rust_analyzer_driver.py refs <file> <line> <column>

Lines and columns are 1-based, as an editor shows them; the column is the
character within the line. Paths are relative to the workspace root.
"""
import json, os, subprocess, sys, threading, time, urllib.parse, queue

ROOT = os.path.abspath(os.environ.get("RA_ROOT", "/home/Slimy/container/programming/Rust/tilesim"))
SERVER = os.path.expanduser("~/.vscode-oss/extensions/rust-lang.rust-analyzer-0.3.3057-linux-x64/server/rust-analyzer")


def uri_of(path):
    return "file://" + urllib.parse.quote(os.path.abspath(os.path.join(ROOT, path)))


def path_of(uri):
    return urllib.parse.unquote(uri[len("file://"):])


class Server:
    def __init__(self):
        self.process = subprocess.Popen([SERVER], cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        self.next_id, self.replies, self.progress_ended = 0, {}, set()
        self.reply_ready = threading.Condition()
        threading.Thread(target=self.read_loop, daemon=True).start()

    def send(self, message):
        body = json.dumps(message).encode()
        self.process.stdin.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)
        self.process.stdin.flush()

    def read_loop(self):
        stream = self.process.stdout
        while True:
            headers = {}
            while True:
                line = stream.readline()
                if not line:
                    return
                line = line.decode().strip()
                if not line:
                    break
                key, value = line.split(":", 1)
                headers[key.lower()] = value.strip()
            message = json.loads(stream.read(int(headers["content-length"])))
            if "id" in message and "method" in message:  # a request from the server
                method = message["method"]
                result = [{}] * len(message["params"]["items"]) if method == "workspace/configuration" else None
                self.send({"jsonrpc": "2.0", "id": message["id"], "result": result})
            elif "id" in message:
                with self.reply_ready:
                    self.replies[message["id"]] = message
                    self.reply_ready.notify_all()
            elif message.get("method") == "$/progress":
                value = message["params"]["value"]
                if value.get("kind") == "end":
                    with self.reply_ready:
                        self.progress_ended.add(str(message["params"]["token"]))
                        self.reply_ready.notify_all()

    def request(self, method, params, timeout=600):
        self.next_id += 1
        request_id = self.next_id
        self.send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        with self.reply_ready:
            if not self.reply_ready.wait_for(lambda: request_id in self.replies, timeout):
                sys.exit(f"no reply to {method}")
            reply = self.replies.pop(request_id)
        if "error" in reply:
            sys.exit(f"{method} failed: {reply['error']}")
        return reply.get("result")

    def notify(self, method, params):
        self.send({"jsonrpc": "2.0", "method": method, "params": params})

    def start(self):
        capabilities = {
            "workspace": {"workspaceEdit": {"documentChanges": True, "resourceOperations": ["create", "rename", "delete"]}, "fileOperations": {"willRename": True}, "configuration": True},
            "window": {"workDoneProgress": True},
            "textDocument": {"rename": {"prepareSupport": True}},
        }
        self.request("initialize", {"processId": os.getpid(), "rootUri": uri_of("."), "capabilities": capabilities, "initializationOptions": {"cargo": {"allTargets": True, "features": "all"}, "checkOnSave": False, "cachePriming": {"enable": True}}})
        self.notify("initialized", {})
        # Indexing done: the cache priming's progress ends (or a quiet spell, if the server says none).
        deadline = time.time() + 900
        with self.reply_ready:
            self.reply_ready.wait_for(lambda: any("cachePriming" in token or "Indexing" in token for token in self.progress_ended), max(1, deadline - time.time()))
        time.sleep(2)

    def open(self, path):
        with open(os.path.join(ROOT, path)) as source:
            self.notify("textDocument/didOpen", {"textDocument": {"uri": uri_of(path), "languageId": "rust", "version": 1, "text": source.read()}})

    def stop(self):
        try:
            self.request("shutdown", None, timeout=30)
            self.notify("exit", None)
        except SystemExit:
            pass
        self.process.kill()


def utf16_offset(line_text, character):
    """The index into `line_text` (a str) of the UTF-16 offset `character`."""
    units = 0
    for index, char in enumerate(line_text):
        if units >= character:
            return index
        units += 2 if ord(char) > 0xFFFF else 1
    return len(line_text)


def apply_text_edits(path, edits):
    with open(path) as source:
        text = source.read()
    lines = text.split("\n")
    starts = [0]
    for line in lines[:-1]:
        starts.append(starts[-1] + len(line) + 1)

    def offset(position):
        line = position["line"]
        if line >= len(lines):
            return len(text)
        return starts[line] + utf16_offset(lines[line], position["character"])

    for edit in sorted(edits, key=lambda edit: (edit["range"]["start"]["line"], edit["range"]["start"]["character"]), reverse=True):
        start, end = offset(edit["range"]["start"]), offset(edit["range"]["end"])
        text = text[:start] + edit["newText"] + text[end:]
    with open(path, "w") as target:
        target.write(text)


def apply_workspace_edit(workspace_edit):
    touched = []
    for change in workspace_edit.get("documentChanges", []):
        kind = change.get("kind")
        if kind == "rename":
            old, new = path_of(change["oldUri"]), path_of(change["newUri"])
            os.makedirs(os.path.dirname(new), exist_ok=True)
            subprocess.run(["git", "mv", old, new], cwd=ROOT, check=False) if subprocess.run(["git", "ls-files", "--error-unmatch", old], cwd=ROOT, capture_output=True).returncode == 0 else os.rename(old, new)
            touched.append(f"moved {os.path.relpath(old, ROOT)} -> {os.path.relpath(new, ROOT)}")
        elif kind == "create":
            path = path_of(change["uri"])
            os.makedirs(os.path.dirname(path), exist_ok=True)
            open(path, "a").close()
            touched.append(f"created {os.path.relpath(path, ROOT)}")
        elif kind == "delete":
            path = path_of(change["uri"])
            os.remove(path)
            touched.append(f"deleted {os.path.relpath(path, ROOT)}")
        else:
            path = path_of(change["textDocument"]["uri"])
            apply_text_edits(path, change["edits"])
            touched.append(f"edited {os.path.relpath(path, ROOT)} ({len(change['edits'])})")
    for uri, edits in (workspace_edit.get("changes") or {}).items():
        apply_text_edits(path_of(uri), edits)
        touched.append(f"edited {os.path.relpath(path_of(uri), ROOT)} ({len(edits)})")
    return touched


def main():
    command, arguments = sys.argv[1], sys.argv[2:]
    server = Server()
    server.start()
    try:
        if command == "rename":
            for index in range(0, len(arguments), 4):
                path, line, column, new_name = arguments[index:index + 4]
                server.open(path)
                position = {"line": int(line) - 1, "character": int(column) - 1}
                edit = server.request("textDocument/rename", {"textDocument": {"uri": uri_of(path)}, "position": position, "newName": new_name})
                print("\n".join(apply_workspace_edit(edit or {})) or f"nothing renamed at {path}:{line}:{column}")
        elif command == "move":
            for index in range(0, len(arguments), 2):
                old, new = arguments[index:index + 2]
                edit = server.request("workspace/willRenameFiles", {"files": [{"oldUri": uri_of(old), "newUri": uri_of(new)}]})
                touched = apply_workspace_edit(edit or {})
                os.makedirs(os.path.dirname(os.path.join(ROOT, new)), exist_ok=True)
                tracked = subprocess.run(["git", "ls-files", "--error-unmatch", old], cwd=ROOT, capture_output=True).returncode == 0
                subprocess.run(["git", "mv", old, new], cwd=ROOT, check=True) if tracked else os.rename(os.path.join(ROOT, old), os.path.join(ROOT, new))
                print("\n".join(touched + [f"moved {old} -> {new}"]))
        elif command == "refs":
            path, line, column = arguments
            server.open(path)
            found = server.request("textDocument/references", {"textDocument": {"uri": uri_of(path)}, "position": {"line": int(line) - 1, "character": int(column) - 1}, "context": {"includeDeclaration": True}})
            for location in found or []:
                print(f"{os.path.relpath(path_of(location['uri']), ROOT)}:{location['range']['start']['line'] + 1}")
        else:
            sys.exit(__doc__)
    finally:
        server.stop()


if __name__ == "__main__":
    main()
