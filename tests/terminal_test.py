#!/usr/bin/env python3
"""Black-box terminal acceptance checks: python3 terminal_test.py /path/to/folderwalk."""
import codecs
import fcntl
import os
import pathlib
import pty
import select
import struct
import subprocess
import sys
import termios
import time
import unicodedata

WIDTH, HEIGHT = 220, 24
BINARY = pathlib.Path(sys.argv[1]).resolve() if len(sys.argv) == 2 else None
ROOT = pathlib.Path(os.environ.get('TMPDIR', str(pathlib.Path.home() / '.scratch/tmp')))
ROOT.mkdir(parents=True, exist_ok=True)
FIXTURE = pathlib.Path(__import__('tempfile').mkdtemp(prefix='fixture-', dir=ROOT))
BASE = FIXTURE / 'start'
CHILD = BASE / 'a child'
UNICODE = BASE / 'český prostor'
OUTSIDE = FIXTURE / 'physical parent' / 'target'
for path in (CHILD, UNICODE, OUTSIDE, BASE / 'other'):
    path.mkdir(parents=True, exist_ok=True)
(BASE / '.hidden').write_text('hidden\n', encoding='utf-8')
(BASE / 'file.txt').write_text('file\n', encoding='utf-8')
(BASE / 'link').symlink_to(OUTSIDE, target_is_directory=True)


def physical(path, cwd=BASE):
    env = os.environ.copy()
    env.pop('CDPATH', None)
    result = subprocess.run(['bash', '--noprofile', '--norc', '-c',
                             'cd -P -- "$1" && pwd -P', 'oracle', str(path)],
                            cwd=cwd, env=env, capture_output=True, text=True, check=True)
    return result.stdout.rstrip('\n')


class Screen:
    """Track printable cells and ANSI positioning/erasure; retain main and alternate buffers."""
    def __init__(self):
        self.main = [[' '] * WIDTH for _ in range(HEIGHT)]
        self.alt = [[' '] * WIDTH for _ in range(HEIGHT)]
        self.cells = self.main
        self.row = self.col = 0
        self.saved = (0, 0)
        self.state = 'text'
        self.seq = ''
        self.decoder = codecs.getincrementaldecoder('utf-8')('replace')

    def feed(self, data):
        for ch in self.decoder.decode(data):
            self.character(ch)

    def character(self, ch):
        if self.state == 'osc':
            if ch == '\x07':
                self.state = 'text'
            elif ch == '\x1b':
                self.state = 'osc_escape'
            return
        if self.state == 'osc_escape':
            self.state = 'text' if ch == '\\' else 'osc'
            return
        if self.state == 'escape':
            if ch == '[':
                self.state, self.seq = 'csi', ''
            elif ch == ']':
                self.state = 'osc'
            else:
                if ch == '7': self.saved = (self.row, self.col)
                if ch == '8': self.row, self.col = self.saved
                self.state = 'text'
            return
        if self.state == 'csi':
            if '@' <= ch <= '~':
                self.csi(self.seq, ch)
                self.state = 'text'
            else:
                self.seq += ch
            return
        if ch == '\x1b': self.state = 'escape'; return
        if ch == '\r': self.col = 0; return
        if ch == '\n': self.row = min(HEIGHT - 1, self.row + 1); return
        if ch == '\b': self.col = max(0, self.col - 1); return
        if ord(ch) < 32 or ord(ch) == 127: return
        width = 0 if unicodedata.combining(ch) else (2 if unicodedata.east_asian_width(ch) in 'WF' else 1)
        if width == 0:
            if self.col: self.cells[self.row][self.col - 1] += ch
            return
        if self.col >= WIDTH:
            self.row = min(HEIGHT - 1, self.row + 1)
            self.col = 0
        self.cells[self.row][self.col] = ch
        if width == 2 and self.col + 1 < WIDTH: self.cells[self.row][self.col + 1] = ''
        self.col += width

    def csi(self, raw, final):
        private = raw.startswith('?')
        if private:
            if raw in ('?1049', '?47', '?1047') and final == 'h':
                self.alt = [[' '] * WIDTH for _ in range(HEIGHT)]
                self.cells, self.row, self.col = self.alt, 0, 0
            elif raw in ('?1049', '?47', '?1047') and final == 'l':
                self.cells, self.row, self.col = self.main, 0, 0
            return
        try:
            nums = [int(x) if x else 0 for x in raw.split(';')]
        except ValueError:
            return
        n = nums[0] or 1
        if final in 'Hf':
            self.row = min(HEIGHT - 1, max(0, (nums[0] or 1) - 1))
            self.col = min(WIDTH - 1, max(0, (nums[1] if len(nums) > 1 and nums[1] else 1) - 1))
        elif final == 'A': self.row = max(0, self.row - n)
        elif final == 'B': self.row = min(HEIGHT - 1, self.row + n)
        elif final == 'C': self.col = min(WIDTH - 1, self.col + n)
        elif final == 'D': self.col = max(0, self.col - n)
        elif final == 'G': self.col = min(WIDTH - 1, n - 1)
        elif final == 'd': self.row = min(HEIGHT - 1, n - 1)
        elif final == 's': self.saved = (self.row, self.col)
        elif final == 'u': self.row, self.col = self.saved
        elif final == 'J':
            mode = nums[0]
            if mode in (2, 3):
                for r in range(HEIGHT): self.cells[r] = [' '] * WIDTH
            elif mode == 0:
                for c in range(min(self.col, WIDTH), WIDTH): self.cells[self.row][c] = ' '
                for r in range(self.row + 1, HEIGHT): self.cells[r] = [' '] * WIDTH
            elif mode == 1:
                for r in range(self.row): self.cells[r] = [' '] * WIDTH
                for c in range(min(self.col + 1, WIDTH)): self.cells[self.row][c] = ' '
        elif final == 'K':
            mode = nums[0]
            left, right = (0, WIDTH) if mode == 2 else ((0, min(self.col + 1, WIDTH)) if mode == 1 else (min(self.col, WIDTH), WIDTH))
            for c in range(left, right): self.cells[self.row][c] = ' '

    def rows(self):
        return [''.join(row).rstrip() for row in self.cells]


class Terminal:
    def __init__(self):
        self.master, self.slave = pty.openpty()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack('HHHH', HEIGHT, WIDTH, 0, 0))
        self.before = termios.tcgetattr(self.slave)
        self.screen = Screen()
        self.raw = bytearray()
        env = os.environ.copy()
        env.update({'TERM': 'xterm-256color', 'LC_ALL': 'C.UTF-8'})
        env.pop('CDPATH', None)
        script = 'printf "SHELL_BEFORE=%s\\n" "$PWD"; "$1"; r=$?; printf "SHELL_AFTER=%s\\n" "$PWD"; exit "$r"'
        self.proc = subprocess.Popen(['bash', '--noprofile', '--norc', '-c', script, 'shell', str(BINARY)],
                                     cwd=BASE, stdin=self.slave, stdout=self.slave, stderr=self.slave,
                                     close_fds=True, env=env)
        try:
            self.wait_for(lambda: self.screen.rows()[0] == physical(BASE), 'startup header')
        except Exception:
            self.close()
            raise

    def drain(self, timeout=0.1):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            ready, _, _ = select.select([self.master], [], [], max(0, deadline - time.monotonic()))
            if not ready: break
            try: data = os.read(self.master, 65536)
            except OSError: break
            if not data: break
            self.raw.extend(data)
            self.screen.feed(data)

    def wait_for(self, pred, label, timeout=2.0):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.drain(0.05)
            if pred(): return
            if self.proc.poll() is not None: break
        raise AssertionError(f'{label}: actual rows={self.screen.rows()!r}; process={self.proc.poll()}')

    def send(self, data):
        os.write(self.master, data)
        self.drain(0.15)

    def rows(self):
        self.drain(0.1)
        return self.screen.rows()

    def path(self): return self.rows()[0]
    def view(self): return self.rows()[:-2]
    def entries(self):
        rows = self.rows()[1:-2]
        result = []
        for row in rows:
            if row.startswith('> '): row = row[2:]
            elif row.startswith('  '): row = row[2:]
            else: continue
            if row.endswith(('/', '@')): row = row[:-1]
            if row: result.append(row)
        return result

    def quit(self, keys=b'q'):
        os.write(self.master, keys)
        deadline = time.monotonic() + 3
        while self.proc.poll() is None and time.monotonic() < deadline: self.drain(0.05)
        self.drain(0.2)
        if self.proc.poll() is None:
            self.proc.kill()
            self.proc.wait()
            raise AssertionError('application did not exit')
        return self.proc.returncode, termios.tcgetattr(self.slave), bytes(self.raw)

    def close(self):
        if self.proc.poll() is None:
            self.proc.kill()
            self.proc.wait()
        os.close(self.master)
        os.close(self.slave)


def check_equal(actual, expected, label):
    if actual != expected: raise AssertionError(f'{label}: expected={expected!r}, actual={actual!r}')


def submit(term, text):
    term.send(b'g')
    term.send(text.encode('utf-8'))
    term.send(b'\r')


def select_name(term, name):
    rows = term.rows()[1:-2]
    indexed = [(i, r[2:].rstrip('/@')) for i, r in enumerate(rows) if r.startswith(('> ', '  ')) and r[2:]]
    selected = next(i for i, r in enumerate(rows) if r.startswith('> '))
    target = next(i for i, n in indexed if n == name)
    for _ in range(abs(target - selected)):
        term.send(b'\x1b[B' if target > selected else b'\x1b[A')


def startup(t):
    expected = sorted(os.listdir(BASE))
    check_equal(t.path(), physical(BASE), 'full absolute startup path')
    check_equal(sorted(t.entries()), expected, 'entry membership')
    rows = t.rows()[1:-2]
    if not any(row[2:] == 'a child/' for row in rows if row.startswith(('> ', '  '))):
        raise AssertionError(f'directory marker missing: {rows!r}')
    if not any(row[2:] == 'link@' for row in rows if row.startswith(('> ', '  '))):
        raise AssertionError(f'symlink marker missing: {rows!r}')
    return f'path={t.path()!r}; entries={sorted(t.entries())!r}'


def keyboard(t):
    select_name(t, 'a child')
    t.send(b'\x1b[C')
    t.wait_for(lambda: t.path() == physical(CHILD), 'open child with Right')
    t.send(b'\x1b[D')
    t.wait_for(lambda: t.path() == physical(BASE), 'return parent with Left')
    return f'child={physical(CHILD)!r}; parent={t.path()!r}'


def typed(t):
    submit(t, 'a child')
    t.wait_for(lambda: t.path() == physical(CHILD), 'relative space path')
    submit(t, str(UNICODE))
    t.wait_for(lambda: t.path() == physical(UNICODE), 'absolute Unicode space path')
    submit(t, '../a child')
    t.wait_for(lambda: t.path() == physical(CHILD), 'relative parent path')
    return f'relative={physical(CHILD)!r}; absolute={physical(UNICODE)!r}'


def symlink(t):
    select_name(t, 'link')
    t.send(b'\r')
    t.wait_for(lambda: t.path() == physical(OUTSIDE), 'symlink target')
    t.send(b'\x7f')
    t.wait_for(lambda: t.path() == physical(OUTSIDE.parent), 'physical parent')
    return f'target={physical(OUTSIDE)!r}; parent={t.path()!r}'


def error_recovery(t):
    env = os.environ.copy()
    env.pop('CDPATH', None)
    oracle = subprocess.run(
        ['bash', '--noprofile', '--norc', '-c',
         'before=$(pwd -P); cd -P -- "$1" 2>/dev/null; status=$?; '
         'after=$(pwd -P); printf "%s\\n%s\\n%s\\n" "$status" "$before" "$after"',
         'oracle', 'a childX'],
        cwd=BASE, env=env, capture_output=True, text=True, check=True,
    )
    lines = oracle.stdout.splitlines()
    if len(lines) != 3:
        raise AssertionError(f'Bash missing-path output: {oracle.stdout!r}')
    status, before, after = lines
    if int(status) == 0:
        raise AssertionError('Bash accepted the missing path')
    check_equal(before, physical(BASE), 'Bash initial directory')
    check_equal(after, before, 'Bash directory after failed cd')
    prior = t.view()
    t.send(b'g'); t.send(b'a childX'); t.send(b'\r')
    rows = t.rows()
    check_equal(t.view(), prior, 'failed navigation keeps path and list')
    if not any('Path: a childX' in r for r in rows[-2:]): raise AssertionError(f'input not retained: {rows!r}')
    if not rows[-2].strip(): raise AssertionError(f'error not visible: {rows!r}')
    t.send(b'\x7f'); t.send(b'\r')
    t.wait_for(lambda: t.path() == physical(CHILD), 'recovery after editing')
    return f'Bash cd status={status}, unchanged pwd={after!r}; error={rows[-2]!r}; recovered={t.path()!r}'


def steady(t):
    select_name(t, 'other')
    prior = t.view()
    t.send(b'g'); t.send('český'.encode()); t.send(b'\x7f')
    check_equal(t.view(), prior, 'edit preserves path, selection and listing')
    if not any('Path: česk' in r for r in t.rows()[-2:]): raise AssertionError(f'editing not visible: {t.rows()!r}')
    t.send(b'\x1b'); time.sleep(0.3); t.drain(0.2)
    check_equal(t.view(), prior, 'cancel preserves view')
    return 'path, selected row and viewport unchanged during editing and cancel'


def paste_split(t):
    prior = t.view()
    payload = str(UNICODE).encode()
    cut = payload.index('č'.encode()) + 1
    for part in (b'\x1b[2', b'00~', payload[:cut], payload[cut:], b'\x1b[20', b'1~'):
        t.send(part)
        check_equal(t.view(), prior, 'paste cannot navigate before explicit Enter')
    if not any('Path: ' in r for r in t.rows()[-2:]): raise AssertionError('paste did not open editor')
    t.send(b'\r')
    t.wait_for(lambda: t.path() == physical(UNICODE), 'split UTF-8 paste submitted')
    return f'split markers and Unicode remained pending; Enter opened {t.path()!r}'


def paste_control(t):
    prior = t.view()
    t.send(b'\x1b[200~a child\n\x1bq\x1b[201~')
    check_equal(t.view(), prior, 'control paste did not navigate or quit')
    if t.proc.poll() is not None: raise AssertionError('control paste quit')
    rows = t.rows()
    if not rows[-2].strip(): raise AssertionError(f'control paste error missing: {rows!r}')
    if not any(r.startswith('Path:') for r in rows[-2:]):
        raise AssertionError(f'editor was cancelled: {rows!r}')
    t.send(b'a child'); t.send(b'\r')
    t.wait_for(lambda: t.path() == physical(CHILD), 'recovery from control paste')
    return f'no early navigation; error={rows[-2]!r}; editable recovery={t.path()!r}'


def exit_check(t):
    before = physical(BASE)
    code, after, raw = t.quit()
    check_equal(code, 0, 'quit exit')
    check_equal(after, t.before, 'terminal attributes restored')
    for marker in (f'SHELL_BEFORE={before}', f'SHELL_AFTER={before}'):
        if marker.encode() not in raw: raise AssertionError(f'missing shell marker {marker!r}')
    return f'exit={code}; terminal attributes restored; shell before and after={before!r}'


def edit_ctrl_c(t):
    t.send(b'g'); t.send(b'not a path')
    code, after, _ = t.quit(b'\x03')
    check_equal(code, 0, 'Ctrl+C exit in editor')
    check_equal(after, t.before, 'terminal attributes restored after Ctrl+C')
    return f'editor Ctrl+C exit={code}; terminal attributes restored'


def main():
    if BINARY is None or not BINARY.is_file():
        print('usage: python3 terminal_test.py /path/to/folderwalk', file=sys.stderr)
        return 2
    tests = [
        ('startup', 'fixture with hidden file, directory, file and symlink', 'physical absolute path and all names', startup),
        ('keyboard', 'select child, Right, Left', 'child then physical parent', keyboard),
        ('typed', 'relative and absolute paths with spaces and Unicode', 'Bash physical paths', typed),
        ('symlink', 'select directory symlink, Enter, Backspace', 'target and target physical parent', symlink),
        ('error', 'invalid path then Backspace and Enter', 'error and stable view, followed by recovery', error_recovery),
        ('edit', 'select row, edit Unicode and cancel', 'path and all list rows remain unchanged', steady),
        ('paste', 'split framing and UTF-8, then separate Enter', 'no early navigation; Enter opens path', paste_split),
        ('paste controls', 'framed newline and Escape, then Enter', 'no early navigation or quit; visible error', paste_control),
        ('exit', 'quit through shell wrapper', 'exit 0, terminal attributes and shell directory restored', exit_check),
        ('editor exit', 'edit then Ctrl+C', 'exit 0 and terminal attributes restored', edit_ctrl_c),
    ]
    passed = 0
    for name, inputs, expected, test in tests:
        term = None
        try:
            term = Terminal()
            actual = test(term)
            passed += 1
            print(f'PASS {name} | input: {inputs} | expected: {expected} | actual: {actual}', flush=True)
        except Exception as exc:
            print(f'FAIL {name} | input: {inputs} | expected: {expected} | actual: {exc}', flush=True)
        finally:
            if term is not None: term.close()
    print(f'RESULT {passed} passed, {len(tests) - passed} failed, {len(tests)} total; fixture={FIXTURE}')
    return 0 if passed == len(tests) else 1


if __name__ == '__main__':
    sys.exit(main())
