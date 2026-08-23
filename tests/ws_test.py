#!/usr/bin/env python3
"""端到端测试：连接 /ws，模拟浏览器 WebSocket 客户端验证 Web 终端。"""
import base64, os, socket, struct, sys

HOST, PORT = "127.0.0.1", 8080
s = socket.create_connection((HOST, PORT))
key = base64.b64encode(os.urandom(16)).decode()
req = (
    "GET /ws HTTP/1.1\r\n"
    f"Host: {HOST}:{PORT}\r\n"
    "Upgrade: websocket\r\n"
    "Connection: Upgrade\r\n"
    f"Sec-WebSocket-Key: {key}\r\n"
    "Sec-WebSocket-Version: 13\r\n\r\n"
)
s.sendall(req.encode())
resp = b""
while b"\r\n\r\n" not in resp:
    resp += s.recv(1024)
print("SERVER HANDSHAKE:")
print(resp.decode(errors="replace").rstrip())
assert b"101" in resp.split(b"\r\n", 1)[0], "upgrade failed"

def send_masked(sock, opcode, payload):
    mask = os.urandom(4)
    b0 = 0x80 | opcode
    b1 = 0x80
    ln = len(payload)
    if ln < 126:
        hdr = bytes([b0, b1 | ln])
    elif ln < 65536:
        hdr = bytes([b0, b1 | 126]) + struct.pack(">H", ln)
    else:
        hdr = bytes([b0, b1 | 127]) + struct.pack(">Q", ln)
    masked = bytes(pl ^ mask[i % 4] for i, pl in enumerate(payload))
    sock.sendall(hdr + mask + masked)

def recv_frame(sock):
    def rd(n):
        b = b""
        while len(b) < n:
            c = sock.recv(n - len(b))
            if not c:
                raise EOFError
            b += c
        return b
    h = rd(2)
    op = h[0] & 0x0F
    ln = h[1] & 0x7F
    if ln == 126:
        ln = struct.unpack(">H", rd(2))[0]
    elif ln == 127:
        ln = struct.unpack(">Q", rd(8))[0]
    payload = rd(ln) if ln else b""
    return op, payload

# 1) resize 指令（文本帧，走 /ws 会被当作命令）
send_masked(s, 0x1, b"st\t100\t30")
# 2) 执行两条命令（二进码帧，当作终端输入）
cmd = b"echo hello-from-web-terminal; echo '\xe4\xbd\xa0\xe5\xa5\xbd' ; pwd; uname -a\r\n"
send_masked(s, 0x2, cmd)

out = b""
s.settimeout(2.0)
try:
    while True:
        op, p = recv_frame(s)
        if op == 0x8:
            print("GOT CLOSE FRAME"); break
        out += p
except (socket.timeout, EOFError):
    pass

print("\n=== PTY OUTPUT (raw, may include ANSI) ===")
print(out.decode("utf-8", errors="replace"))
assert b"hello-from-web-terminal" in out, "did not see command output"
assert b"\xe4\xbd\xa0\xe5\xa5\xbd" in out, "did not see UTF-8 output"
print("\n[PASS] Web 终端双向联通正常")
s.close()