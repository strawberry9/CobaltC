#!/usr/bin/env python3
"""Reports 032/037 on a real Windows machine (CHG-0475, CHG-0476): what `cobc --run` does when its program
runs for ever and when cobc is stopped.  Run by .github/workflows/bootstrap.yml's Windows job (the Linux
cases are impl/tests/cases/cobc; under Wine the console is not a real one).  Usage:
    python impl/tests/windows/cleanup.py PATH-TO-cobc.exe CC
Checks, each by a program that never ends:
  1. COBALTC_CPU_LIMIT=3 ends the program `--run` started (exit status not 0) within 60 s.
  2. A console break (CTRL_BREAK_EVENT to cobc's process group) ends cobc and the program it started, and
     removes the directories cobc made (cobc-obj-* in TEMP).
  3. Killing cobc itself (TerminateProcess, no handler can run) ends the program it started (job object).
Standard library only."""
import ctypes, glob, os, signal, subprocess, sys, tempfile, time

cobc, cc = sys.argv[1], sys.argv[2]
work = tempfile.mkdtemp(prefix="wincase-")
tmp = os.path.join(work, "tmp")
os.mkdir(tmp)
with open(os.path.join(work, "busy.cb"), "w") as f:
    f.write("""import std;

fn main() : u8
{
    u64 x = 1;
    while (x != 0)
    {
        x = x % 1000003 + 1;
    }
    0
}
""")
env = dict(os.environ, TEMP=tmp, TMP=tmp, TMPDIR=tmp)
failed = []


def children_of(pid):
    out = subprocess.run(["powershell", "-NoProfile", "-Command",
                          "Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq %d } | ForEach-Object { $_.ProcessId }" % pid],
                         capture_output=True, text=True).stdout.split()
    return [int(x) for x in out]


def alive(pid):
    out = subprocess.run(["tasklist", "/FI", "PID eq %d" % pid, "/NH"], capture_output=True, text=True).stdout
    return str(pid) in out


def descendants(pid):
    seen, todo = [], [pid]
    while todo:
        p = todo.pop()
        for c in children_of(p):
            if c not in seen:
                seen.append(c)
                todo.append(c)
    return seen


def wait_for(cond, secs):
    end = time.time() + secs
    while time.time() < end:
        if cond():
            return True
        time.sleep(0.5)
    return cond()


def check(name, ok, detail=""):
    print(("ok   " if ok else "FAIL ") + name + (" " + detail if detail else ""), flush=True)
    if not ok:
        failed.append(name)


# 1. the processor time limit
e1 = dict(env, COBALTC_CPU_LIMIT="3")
t0 = time.time()
p = subprocess.Popen([cobc, "--cc", cc, "--run", "busy.cb"], cwd=work, env=e1, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
try:
    out, err = p.communicate(timeout=120)
    check("cpu limit ends the program", p.returncode != 0 and time.time() - t0 < 110,
          "(status %s after %.0f s) %s" % (p.returncode, time.time() - t0, err.decode(errors="replace")[:200]))
except subprocess.TimeoutExpired:
    p.kill()
    check("cpu limit ends the program", False, "(still running after 120 s)")


def start_run():
    p = subprocess.Popen([cobc, "--cc", cc, "--run", "busy.cb"], cwd=work, env=env,
                         stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
                         creationflags=subprocess.CREATE_NEW_PROCESS_GROUP)
    # the program is running when cobc has a child that is not the C compiler: wait for a descendant
    # that stays and whose image is not gcc/clang/cc1/ld/collect2
    def running_program():
        for d in descendants(p.pid):
            name = subprocess.run(["powershell", "-NoProfile", "-Command",
                                   "(Get-Process -Id %d -ErrorAction SilentlyContinue).ProcessName" % d],
                                  capture_output=True, text=True).stdout.strip().lower()
            if name and not any(k in name for k in ("gcc", "clang", "cc1", "collect2", "ld", "as", "lto", "conhost", "cc")):
                return d
        return None
    prog = [None]
    def ready():
        prog[0] = running_program()
        return prog[0] is not None
    if not wait_for(ready, 180):
        p.kill()
        return None, None
    time.sleep(2)
    return p, prog[0]


# 2. a console break
p, prog = start_run()
if p is None:
    check("console break: the program started", False)
else:
    ctypes.windll.kernel32.GenerateConsoleCtrlEvent(1, p.pid)  # CTRL_BREAK_EVENT to cobc's group
    ended = wait_for(lambda: p.poll() is not None, 60)
    check("console break ends cobc", ended)
    if not ended:
        p.kill()
    check("console break ends the program", wait_for(lambda: not alive(prog), 20))
    left = glob.glob(os.path.join(tmp, "cobc-*"))
    check("console break removes cobc's directories", not left, str(left))

# 3. cobc killed outright
p, prog = start_run()
if p is None:
    check("kill: the program started", False)
else:
    p.kill()
    check("killing cobc ends the program (job object)", wait_for(lambda: not alive(prog), 20))
    subprocess.run(["taskkill", "/F", "/T", "/PID", str(prog)], capture_output=True)

print("%d failed" % len(failed))
sys.exit(1 if failed else 0)
