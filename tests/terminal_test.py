#!/usr/bin/env python3
"""Black-box terminal acceptance checks: python3 terminal_test.py /path/to/folderwalk."""
import codecs
import fcntl
import os
import pathlib
import pty
import re
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


def prepare_fixture(case):
    global BASE, CHILD, UNICODE, OUTSIDE, BRANCH
    BASE = case / 'start'
    CHILD = BASE / 'a child'
    UNICODE = BASE / 'český prostor'
    OUTSIDE = case / 'physical parent' / 'target'
    for path in (CHILD, UNICODE, OUTSIDE, BASE / 'other'):
        path.mkdir(parents=True, exist_ok=True)
    (BASE / '.hidden').write_text('hidden\n', encoding='utf-8')
    (BASE / 'file.txt').write_text('file\n', encoding='utf-8')
    (BASE / 'link').symlink_to(OUTSIDE, target_is_directory=True)

    # Four directories below the selectable folder put a real entry past the limit.
    BRANCH = CHILD / 'lab' / 'exercise' / 'solutions'
    (BRANCH / 'private').mkdir(parents=True)
    (CHILD / 'lab' / '.readme').write_text('notes\n')
    (CHILD / 'lab' / 'loop').symlink_to(CHILD, target_is_directory=True)
    (BASE / 'other' / 'sibling.txt').touch()


def physical(path, cwd=None):
    cwd = BASE if cwd is None else cwd
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
    def __init__(self, plain=False):
        self.master, self.slave = pty.openpty()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack('HHHH', HEIGHT, WIDTH, 0, 0))
        self.before = termios.tcgetattr(self.slave)
        self.screen = Screen()
        self.raw = bytearray()
        env = os.environ.copy()
        env.update({'TERM': 'xterm-256color', 'LC_ALL': 'C.UTF-8'})
        if plain:
            env['NO_COLOR'] = '1'
        env.pop('CDPATH', None)
        script = 'printf "SHELL_BEFORE=%s\\n" "$PWD"; "$1"; r=$?; printf "SHELL_AFTER=%s\\n" "$PWD"; exit "$r"'
        self.proc = subprocess.Popen(['bash', '--noprofile', '--norc', '-c', script, 'shell', str(BINARY)],
                                     cwd=BASE, stdin=self.slave, stdout=self.slave, stderr=self.slave,
                                     close_fds=True, env=env)
        try:
            self.wait_for(lambda: self.screen.rows()[0] == physical(BASE), 'startup header')
            self.wait_for(lambda: self.screen.rows()[-3].startswith(('Live', 'Polling')), 'monitor ready', 8)
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
    def view(self): return self.rows()[:-3]
    def entries(self):
        rows = self.rows()[1:-2]
        result = []
        for row in rows:
            if row.startswith('> '): row = row[2:]
            elif row.startswith('  '): row = row[2:]
            else: continue
            name = entry_name(row[2:])  # Skip root disclosure or file padding.
            if name: result.append(name)
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


def entry_name(text):
    for suffix in ('   + Added', '   ~ Edited', '  (empty)', '  (unavailable)'):
        text = text.removesuffix(suffix)
    return text.removesuffix('/').removesuffix('@')


def select_name(term, name, depth=0):
    rows = term.rows()[1:-2]
    selected = next(i for i, row in enumerate(rows) if row.startswith('> '))
    # Strip the selection, indentation and two-cell disclosure before the name.
    offset = 2 + 2 * depth + 2
    target = next(i for i, row in enumerate(rows) if row.startswith(('> ', '  ')) and entry_name(row[offset:]) == name)
    for _ in range(abs(target - selected)):
        term.send(b'\x1b[B' if target > selected else b'\x1b[A')


def startup(t):
    rows = t.view()
    if not any('solutions/' in row for row in rows) or not any('sibling.txt' in row for row in rows):
        raise AssertionError(f'Max startup omitted a branch: {rows!r}')
    t.send(b'm')
    expected = sorted(os.listdir(BASE))
    check_equal(t.path(), physical(BASE), 'full absolute startup path')
    check_equal(sorted(t.entries()), expected, 'entry membership')
    rows = t.rows()[1:-2]
    if not any(row[2:] == '▸ a child/' for row in rows if row.startswith(('> ', '  '))):
        raise AssertionError(f'directory marker missing: {rows!r}')
    if not any(row[2:] == '  link@' for row in rows if row.startswith(('> ', '  '))):
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


def tree_depth(t):
    select_name(t, 'a child')
    rows = t.rows()
    root = rows.index('> ▾ a child/')
    expected = [
        '> ▾ a child/',
        '    ▾ lab/',
        '        .readme',
        '      ▾ exercise/',
        '        · solutions/',
        '        loop@',
    ]
    check_equal(rows[root:root + len(expected)], expected, 'expanded rows, levels, order and link')
    check_equal(t.path(), physical(BASE), 'expand in place')
    select_name(t, 'solutions', 3)
    t.send(b' ')
    check_equal(t.view().count('>       · solutions/'), 1, 'depth-limit directory stays selected')
    if any('private' in row for row in t.view()):
        raise AssertionError('fourth-level entry visible')
    t.send(b'\r')
    check_equal(t.path(), physical(BRANCH), 'open selected depth-limit directory')
    check_equal(t.entries(), ['private'], 'navigate beyond old limit')
    t.send(b'\x7f')
    check_equal(t.path(), physical(BRANCH.parent), 'physical parent of nested directory')
    return 'three levels shown, loop remained a link, Enter opened the boundary directory'


def tree_collapse(t):
    t.send(b'm')
    select_name(t, 'a child')
    t.send(b' ')
    select_name(t, 'other')
    t.send(b' ')
    select_name(t, 'lab', 1)
    t.send(b' ')
    rows = t.view()
    if '>   ▸ lab/' not in rows or any('exercise/' in row for row in rows):
        raise AssertionError(f'nested collapse: {rows!r}')
    t.send(b' ')
    if not any('solutions/' in row for row in t.view()) or any('private/' in row for row in t.view()):
        raise AssertionError(f'reopening nested folder changed the depth limit: {t.view()!r}')
    select_name(t, 'a child')
    t.send(b' ')
    rows = t.view()
    if '> ▸ a child/' not in rows or any('lab/' in row for row in rows):
        raise AssertionError(f'root collapse: {rows!r}')
    if not any('sibling.txt' in row for row in rows):
        raise AssertionError('collapsing a branch also collapsed its sibling')
    return 'nested and root collapse retained selection and the expanded sibling'


def tree_retry(t):
    select_name(t, 'other')
    original = BASE / 'other'
    moved = BASE.parent / 'moved'
    original.rename(moved)
    try:
        t.wait_for(lambda: not any('other/' in row for row in t.screen.rows()[2:-3]),
                   'missing directory disappears from live tree')
    finally:
        moved.rename(original)
    t.wait_for(lambda: any('sibling.txt' in row for row in t.screen.rows()[2:-3]),
               'restored directory appears with its child')
    return 'live tree removed the missing folder and restored its contents on return'


def tree_empty_unavailable(t):
    t.send(b'm')
    empty = BASE / 'other' / 'empty'
    locked = BASE / 'other' / 'locked'
    empty.mkdir()
    locked.mkdir()
    (locked / 'hidden-inside').touch()
    locked.chmod(0)
    try:
        select_name(t, 'other')
        t.send(b' ')
        rows = t.view()
        if not any(row.startswith('    ▾ empty/  (empty)') for row in rows) or '      sibling.txt' not in rows:
            raise AssertionError(f'empty branch or sibling missing: {rows!r}')
        if os.access(locked, os.R_OK):
            return 'empty branch and sibling visible; permission check skipped for privileged process'
        if not any(row.startswith('    ▸ locked/  (unavailable)') for row in rows) or any('hidden-inside' in row for row in rows):
            raise AssertionError(f'unreadable branch: {rows!r}')
        select_name(t, 'locked', 1)
        t.send(b' ')
        locked.chmod(0o700)
        t.wait_for(lambda: any('hidden-inside' in row for row in t.screen.rows()[2:-3]),
                   'unreadable child recovers after permission is restored', 8)
    finally:
        locked.chmod(0o700)
    return 'empty and unreadable folders labelled, sibling visible, unreadable folder retried'


def tree_resize_plain(t):
    select_name(t, 'a child')
    select_name(t, 'solutions', 3)
    fcntl.ioctl(t.slave, termios.TIOCSWINSZ, struct.pack('HHHH', 10, 40, 0, 0))
    t.wait_for(lambda: t.screen.rows()[9].startswith('↑'), 'resize updates the footer')
    rows = t.screen.rows()[:10]
    if '>       · solutions/' not in rows or not any('Level 3' in row for row in rows):
        raise AssertionError(f'selected boundary or hint lost on a narrow screen: {rows!r}')
    if any(len(row) > 40 for row in rows):
        raise AssertionError(f'content exceeded 40 columns: {rows!r}')
    if re.search(rb'\x1b\[[0-9;]*m', t.raw):
        raise AssertionError('found SGR styling in NO_COLOR output')
    fcntl.ioctl(t.slave, termios.TIOCSWINSZ, struct.pack('HHHH', HEIGHT, WIDTH, 0, 0))
    t.wait_for(lambda: t.screen.rows()[0] == physical(BASE), 'restore original width')
    if '>       · solutions/' not in t.view():
        raise AssertionError('resizing the terminal lost the selection')
    return 'plain selection, indentation and boundary remained visible at 40×10 and full width'


def tree_scroll(t):
    t.send(b'm')
    # Create children after startup so expansion must read the selected directory.
    for n in range(32):
        (UNICODE / f'folder-{n:02}').mkdir()
    select_name(t, UNICODE.name)
    t.send(b' ')
    t.send(b'\x1b[B' * 32)
    if not any(row.startswith('> ') and 'folder-31/' in row for row in t.view()):
        raise AssertionError(f'selection outside the scrolled viewport: {t.view()!r}')
    t.send(b'\x1b[C')
    check_equal(t.path(), physical(UNICODE / 'folder-31'), 'Right opens scrolled nested selection')
    return 'Down reached the last of 32 descendants and Right opened its physical path'


def tree_expand_at_bottom(t):
    t.send(b'm')
    shelf = FIXTURE / 'shelf'
    for index in range(32):
        (shelf / f'course-{index:02}').mkdir(parents=True)
    (shelf / 'course-25' / 'lecture.txt').touch()
    submit(t, str(shelf))
    t.send(b'\x1b[B' * 25)
    entries = [row for row in t.view() if row.startswith(('  ', '> '))]
    check_equal(entries[-1], '> ▸ course-25/', 'selected folder starts at bottom of viewport')
    t.send(b' ')
    rows = t.view()
    index = rows.index('> ▾ course-25/')
    check_equal(rows[index + 1:index + 2], ['      lecture.txt'], 'first child immediately below focus')
    return 'expanding the last visible row revealed its child and kept its parent selected'


def activity(t, label, path):
    name = str(path)
    t.wait_for(lambda: label in t.screen.rows()[-3] and t.screen.rows()[-3].endswith(name),
               f'{label} activity for {name}', 8)


def selected(t):
    return next(row for row in t.view()[2:] if row.startswith('> '))


def live_file_changes(t):
    t.send(b'c')
    if not any('Waiting for a file change' in row for row in t.view()):
        raise AssertionError('startup inventory populated recent changes')
    t.send(b'\x1b')
    select_name(t, 'other')
    path = BASE / '0-lecture'
    delays = []
    def observe(label, changed, action):
        start = time.monotonic()
        action()
        activity(t, label, changed.name)
        delays.append(round((time.monotonic() - start) * 1000))
    observe('Added', path, lambda: path.write_text('abc'))
    if not any('0-lecture   + Added' in row for row in t.view()):
        raise AssertionError('new file has no tree label')
    if 'other/' not in selected(t):
        raise AssertionError('inserting before selection moved focus')
    observe('Edited', path, lambda: path.write_text('def'))
    if not any('0-lecture   ~ Edited' in row for row in t.view()):
        raise AssertionError('same-size file edit has no tree label')
    marker = BASE / 'marker'
    observe('Added', marker, marker.touch)
    replacement = BASE.parent / 'replacement'
    replacement.write_text('ghi')
    observe('Edited', path, lambda: replacement.replace(path))
    t.send(b'c')
    rows = t.view()
    records = [row for row in rows[2:] if row.endswith('0-lecture')]
    check_equal(len(records), 1, 'two edits produce a single path record')
    if not records[0].startswith('> ') or 'Edited' not in records[0]:
        raise AssertionError(f'most recent atomic save not selected: {records!r}')
    t.send(b'\r')
    t.wait_for(lambda: any(row.startswith('> ') and '0-lecture' in row for row in t.screen.rows()[2:-3]),
               'reveal selected file')
    observe('Removed', path, lambda: path.rename(BASE.parent / 'archived'))
    if any('0-lecture' in row for row in t.view()[2:]):
        raise AssertionError('removed entry still in tree')
    return f'visible activity delay in milliseconds for add, save, add, replace, remove: {delays}'


def live_navigation(t):
    t.send(b'\t')
    if 'a child/' not in selected(t): raise AssertionError('Tab missed first folder')
    t.send(b' ')
    t.send(b'\t')
    if 'other/' not in selected(t): raise AssertionError('Tab missed next top-level folder')
    t.send(b'\x1b[Z')
    if 'a child/' not in selected(t): raise AssertionError('Shift+Tab missed previous folder')
    t.send(b'\t\r')
    t.wait_for(lambda: t.path() == physical(BASE / 'other'), 'open second location')
    arrival = CHILD / 'lab' / 'arrival'
    relative = arrival.relative_to(BASE)
    arrival.write_text('one')
    activity(t, 'Added', relative)
    arrival.write_text('two')
    activity(t, 'Edited', relative)
    replacement = BASE.parent / 'saved'
    replacement.write_text('six')
    signal = CHILD / 'ready'
    signal.touch()
    activity(t, 'Added', signal.relative_to(BASE))
    replacement.replace(arrival)
    activity(t, 'Edited', relative)
    arrival.rename(BASE.parent / 'archived')
    activity(t, 'Removed', relative)
    arrival.touch()
    activity(t, 'Added', relative)
    t.send(b'-')
    t.wait_for(lambda: t.path() == physical(BASE), 'return to previous place')
    if 'other/' not in selected(t) or any('arrival' in row for row in t.view()):
        raise AssertionError(f'previous selection or fold was lost: {t.view()!r}')
    select_name(t, 'a child')
    t.send(b' ')
    if not any('arrival' in row and 'Added' in row for row in t.view()):
        raise AssertionError('reopening a folded branch missed the new child')
    t.send(b'm')
    submit(t, str(OUTSIDE))
    t.wait_for(lambda: t.path() == physical(OUTSIDE) and t.screen.rows()[-3].startswith(('Live','Polling')),
               'external displayed directory is ready')
    if not t.view()[1].startswith('Compact'):
        raise AssertionError('navigation changed the selected Compact mode')
    (BASE / 'still-watching').touch()
    activity(t, 'Added', 'still-watching')
    external = OUTSIDE / 'outside-file'
    external.touch()
    activity(t, 'Added', external)
    t.send(b'c\r')
    if 'outside-file' not in selected(t):
        raise AssertionError('external changed file reveal missed focus')
    t.send(b'\x1b[D')
    if 'target/' not in selected(t):
        raise AssertionError('parent navigation did not focus departed child')
    return 'jumped across siblings, restored folds and focus, monitored both launch and external view'


def live_editor(t):
    select_name(t, 'other')
    t.send(b'g')
    t.send(b'unsubmitted')
    prior = t.view()
    (BASE / 'file.txt').write_text('new contents')
    activity(t, 'Edited', 'file.txt')
    (BASE / '0-new').touch()
    activity(t, 'Added', '0-new')
    check_equal(t.rows()[-1], 'Path: unsubmitted', 'editor buffer after external change')
    check_equal(t.view(), prior, 'tree and selection frozen during editing')
    t.send(b'\x1b')
    t.wait_for(lambda: any('0-new' in row for row in t.screen.rows()[2:-3]), 'apply deferred changes')
    if 'other/' not in selected(t):
        raise AssertionError('refresh after Esc lost focus')
    t.send(b'?')
    if not any('Tab' in row and 'folder' in row for row in t.view()):
        raise AssertionError('help missing folder navigation')
    t.send(b'\x1b')
    if 'other/' not in selected(t):
        raise AssertionError('closing help lost tree focus')
    return 'tree and input stable across external changes, caught up on Esc, help retained focus'


def live_root_recovery(t):
    moved = BASE.parent / 'moved-root'
    BASE.rename(moved)
    try:
        t.wait_for(lambda: 'unavailable' in t.screen.rows()[-3].lower(), 'root unavailable', 8)
        t.send(b'c')
        if not any('Waiting for a file change' in row for row in t.view()):
            raise AssertionError('root outage reported false changes')
        t.send(b'\x1b')
    finally:
        moved.rename(BASE)
    t.wait_for(lambda: t.screen.rows()[-3].startswith(('Live', 'Polling'))
               and 'unavailable' not in t.screen.rows()[-3].lower(), 'root available again', 8)
    (BASE / 'after-restore').touch()
    activity(t, 'Added', 'after-restore')
    return 'missing root marked unavailable, history stayed empty, monitoring resumed'


def live_scope_burst(t):
    (BRANCH / 'private' / 'beyond-limit').touch()
    (BASE / 'link' / 'symlink-target-only').touch()
    for index in range(8):
        (BASE / f'note-{index:02}').touch()
    t.wait_for(lambda: all(any(f'note-{i:02}   + Added' in row for row in t.screen.rows()[2:-3])
                           for i in range(8)), 'settled burst in tree', 8)
    t.send(b'c')
    seen = [row for row in t.view()[2:] if row.startswith(('> ', '  '))]
    check_equal(sorted(row.split()[-1] for row in seen), [f'note-{i:02}' for i in range(8)],
                'burst retained with no changes from below the limit or across a symlink')
    for index in range(70):
        (BASE / f'item-{index:02}').touch()
    t.wait_for(lambda: '64 paths' in t.screen.rows()[1], 'history reaches its documented limit', 8)
    last = BASE / 'latest-save'
    last.touch()
    activity(t, 'Added', last.name)
    t.send(b'\x1b[H')
    if not selected(t).endswith(last.name):
        raise AssertionError('Home missed newest path after the bounded list overflowed')
    first = selected(t)
    t.send(b'\x1b[6~')
    if first == selected(t): raise AssertionError('Page Down did not move through recent changes')
    t.send(b'\x1b[5~')
    check_equal(selected(t), first, 'Page Up returns to first change')
    return 'burst retained, deeper writes omitted, history capped at 64 with newest path reachable'


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
        ('tree depth', 'expand a four-level fixture, toggle and enter its boundary', 'three levels and navigable boundary', tree_depth),
        ('tree collapse', 'expand two branches, close and reopen a child, close its parent', 'stable selection and independent siblings', tree_collapse),
        ('tree retry', 'rename selected directory and restore', 'live removal and recovery', tree_retry),
        ('tree empty and unavailable', 'empty and permission-denied children; retry', 'visible siblings and labelled branches', tree_empty_unavailable),
        ('tree resize without color', 'shrink expanded tree to 40 columns by 10 rows, then enlarge', 'plain focus and depth hint remain visible', tree_resize_plain),
        ('tree scroll', 'expand 32 children, move to last and open', 'selected nested path follows scrolling', tree_scroll),
        ('tree expand at bottom', 'expand last visible row of a scrolled list', 'parent and first child both visible', tree_expand_at_bottom),
        ('live operations', 'create, same-size save, atomic replace and remove', 'labels, stable focus, deduplication and reveal', live_file_changes),
        ('live navigation', 'jump, fold, navigate outside and return', 'launch and displayed paths monitored', live_navigation),
        ('live editing', 'change files while editing and open help', 'stable tree, input and focus', live_editor),
        ('live root recovery', 'move monitored root away and back', 'unavailable status and recovered monitoring', live_root_recovery),
        ('live bounds and burst', 'eight files plus two out-of-scope writes', 'eight tree labels and recent records', live_scope_burst),
        ('exit', 'quit through shell wrapper', 'exit 0, terminal attributes and shell directory restored', exit_check),
        ('editor exit', 'edit then Ctrl+C', 'exit 0 and terminal attributes restored', edit_ctrl_c),
    ]
    passed = 0
    for name, inputs, expected, test in tests:
        term = None
        try:
            prepare_fixture(FIXTURE / test.__name__)
            term = Terminal(plain=test is tree_resize_plain)
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
