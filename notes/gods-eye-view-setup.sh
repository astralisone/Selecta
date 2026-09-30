#!/usr/bin/env bash
#
# God's Eye View — install, configure and launch, doing as much as this machine
# allows. Companion to notes/gods-eye-view-setup.md.
#
# Each step reports OK / SKIP / FAIL and the script keeps going where a failure
# is survivable, so a partial environment still gets you as far as it can. Exit
# status is the number of hard failures.
#
#   ./gods-eye-view-setup.sh                 # set up in ~/gods-eye-view, then start it
#   ./gods-eye-view-setup.sh --no-start      # set up only
#   ./gods-eye-view-setup.sh --check         # report what's there, change nothing
#   ./gods-eye-view-setup.sh --verify        # also run the build and unit suite
#   ./gods-eye-view-setup.sh --dir ~/src/gev --port 5173
#
# Nothing here needs root, and it never overwrites an existing .env.

set -uo pipefail

REPO_URL="https://github.com/bilawalsidhu/gods-eye-view.git"
NODE_FALLBACK="v24.21.0"   # used only if nodejs.org can't be queried
TARGET_DIR="${HOME}/gods-eye-view"
NODE_CACHE="${HOME}/.local/gev-node"
PORT=4173
DO_START=1
DO_VERIFY=0
CHECK_ONLY=0

FAILURES=0
declare -a SUMMARY=()

# --- output ----------------------------------------------------------------

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  C_OK=$'\033[32m'; C_SKIP=$'\033[33m'; C_FAIL=$'\033[31m'; C_DIM=$'\033[2m'; C_OFF=$'\033[0m'
else
  C_OK=; C_SKIP=; C_FAIL=; C_DIM=; C_OFF=
fi

ok()   { printf '%s[ OK ]%s %s\n'   "$C_OK"   "$C_OFF" "$1"; SUMMARY+=("OK|$1"); }
skip() { printf '%s[SKIP]%s %s\n'   "$C_SKIP" "$C_OFF" "$1"; SUMMARY+=("SKIP|$1"); }
fail() { printf '%s[FAIL]%s %s\n'   "$C_FAIL" "$C_OFF" "$1"; SUMMARY+=("FAIL|$1"); FAILURES=$((FAILURES + 1)); }
note() { printf '%s       %s%s\n'   "$C_DIM" "$1" "$C_OFF"; }
step() { printf '\n%s\n' "== $1"; }
have() { command -v "$1" >/dev/null 2>&1; }

usage() { sed -n '2,20p' "$0" | sed 's/^#\{1,2\} \{0,1\}//'; exit 0; }

while [ $# -gt 0 ]; do
  case "$1" in
    --dir)      TARGET_DIR="${2:?--dir needs a path}"; shift 2 ;;
    --port)     PORT="${2:?--port needs a number}"; shift 2 ;;
    --no-start) DO_START=0; shift ;;
    --verify)   DO_VERIFY=1; shift ;;
    --check)    CHECK_ONLY=1; DO_START=0; shift ;;
    -h|--help)  usage ;;
    *) printf 'unknown option: %s (try --help)\n' "$1" >&2; exit 2 ;;
  esac
done

printf 'God'"'"'s Eye View setup — target %s\n' "$TARGET_DIR"
[ "$CHECK_ONLY" -eq 1 ] && note 'check mode: reporting only, nothing will be changed'

# --- node ------------------------------------------------------------------
# GEV pins >=24.14.0 <25 || >=26 <27. Node 25 is EOL and explicitly rejected.

node_ok() {
  local v="${1#v}" major minor patch
  IFS=. read -r major minor patch <<<"$v"
  [ -n "${patch:-}" ] || return 1
  if [ "$major" -eq 24 ]; then
    [ "$minor" -gt 14 ] && return 0
    [ "$minor" -eq 14 ] && [ "$patch" -ge 0 ] && return 0
    return 1
  fi
  [ "$major" -eq 26 ] && return 0
  return 1
}

install_node() {
  # Prefer a version manager the user already has; fall back to an official
  # tarball unpacked under ~/.local. Echoes a bin directory on success.
  local mgr
  for mgr in fnm volta; do
    have "$mgr" || continue
    case "$mgr" in
      fnm)   fnm install 24 >/dev/null 2>&1 && fnm use 24 >/dev/null 2>&1 ;;
      volta) volta install node@24 >/dev/null 2>&1 ;;
    esac
    node_ok "$(node -v 2>/dev/null || echo v0.0.0)" && { echo "$(dirname "$(command -v node)")"; return 0; }
  done

  if [ -x "$NODE_CACHE/bin/node" ] && node_ok "$("$NODE_CACHE/bin/node" -v)"; then
    echo "$NODE_CACHE/bin"; return 0
  fi

  have curl || return 1
  have tar  || return 1

  local os arch ver url tmp
  case "$(uname -s)" in
    Linux)  os=linux ;;
    Darwin) os=darwin ;;
    *) return 1 ;;
  esac
  case "$(uname -m)" in
    x86_64|amd64)  arch=x64 ;;
    arm64|aarch64) arch=arm64 ;;
    *) return 1 ;;
  esac

  ver="$(curl -fsSL --max-time 30 https://nodejs.org/dist/index.json 2>/dev/null \
        | tr '{' '\n' | grep -o '"version":"v24\.[0-9]*\.[0-9]*"' \
        | cut -d'"' -f4 | sort -V | tail -1)"
  [ -n "$ver" ] || ver="$NODE_FALLBACK"

  url="https://nodejs.org/dist/${ver}/node-${ver}-${os}-${arch}.tar.xz"
  tmp="$(mktemp -d)" || return 1
  if ! curl -fsSL --max-time 300 "$url" -o "$tmp/node.tar.xz"; then rm -rf "$tmp"; return 1; fi
  if ! tar -xf "$tmp/node.tar.xz" -C "$tmp"; then rm -rf "$tmp"; return 1; fi
  mkdir -p "$(dirname "$NODE_CACHE")"
  rm -rf "$NODE_CACHE"
  mv "$tmp/node-${ver}-${os}-${arch}" "$NODE_CACHE" || { rm -rf "$tmp"; return 1; }
  rm -rf "$tmp"
  echo "$NODE_CACHE/bin"
}

step 'Node'
CURRENT_NODE="$(node -v 2>/dev/null || echo none)"
if [ "$CURRENT_NODE" != none ] && node_ok "$CURRENT_NODE"; then
  ok "Node $CURRENT_NODE on PATH satisfies the engine pin"
elif [ "$CHECK_ONLY" -eq 1 ]; then
  skip "Node $CURRENT_NODE does not satisfy >=24.14.0 <25 || >=26 <27 (check mode: not installing)"
else
  note "Node $CURRENT_NODE does not satisfy >=24.14.0 <25 || >=26 <27 — getting one"
  if NODE_BIN="$(install_node)" && [ -n "${NODE_BIN:-}" ]; then
    PATH="$NODE_BIN:$PATH"; export PATH
    ok "Node $(node -v) ready ($NODE_BIN)"
    note "add to your shell to keep it: export PATH=\"$NODE_BIN:\$PATH\""
  else
    fail 'could not obtain a supported Node — install Node 24.x or 26.x and re-run'
    note 'nvm users: nvm install 24 && nvm use 24   (nvm is a shell function, not visible to this script)'
    printf '\n%d step(s) failed.\n' "$FAILURES"
    exit "$FAILURES"
  fi
fi
have npm && ok "npm $(npm -v)" || fail 'npm not found alongside node'

# --- checkout --------------------------------------------------------------

step 'Checkout'
if [ -d "$TARGET_DIR/.git" ]; then
  if [ "$CHECK_ONLY" -eq 1 ]; then
    ok "existing checkout at $TARGET_DIR ($(git -C "$TARGET_DIR" rev-parse --short HEAD 2>/dev/null || echo '?'))"
  elif [ -n "$(git -C "$TARGET_DIR" status --porcelain 2>/dev/null)" ]; then
    skip 'checkout has local changes — leaving it alone instead of pulling'
  elif git -C "$TARGET_DIR" pull --ff-only >/dev/null 2>&1; then
    ok "updated to $(git -C "$TARGET_DIR" rev-parse --short HEAD)"
  else
    skip 'could not fast-forward (offline, or diverged) — using the checkout as-is'
  fi
elif [ -e "$TARGET_DIR" ]; then
  fail "$TARGET_DIR exists and is not a git checkout — pass --dir somewhere else"
  printf '\n%d step(s) failed.\n' "$FAILURES"
  exit "$FAILURES"
elif [ "$CHECK_ONLY" -eq 1 ]; then
  skip "no checkout at $TARGET_DIR (check mode: not cloning)"
  printf '\nCheck complete.\n'
  exit 0
elif git clone --depth 1 "$REPO_URL" "$TARGET_DIR" >/dev/null 2>&1; then
  ok "cloned to $TARGET_DIR ($(git -C "$TARGET_DIR" rev-parse --short HEAD))"
else
  fail "clone failed — check network access to github.com"
  printf '\n%d step(s) failed.\n' "$FAILURES"
  exit "$FAILURES"
fi

cd "$TARGET_DIR" || { fail "cannot enter $TARGET_DIR"; exit 1; }

# --- dependencies ----------------------------------------------------------

step 'Dependencies'
if [ "$CHECK_ONLY" -eq 1 ]; then
  [ -d node_modules ] && ok 'node_modules present' || skip 'dependencies not installed (check mode)'
elif npm ci --no-audit --no-fund >/tmp/gev-npm-ci.log 2>&1; then
  ok "installed ($(grep -oE 'added [0-9]+ packages' /tmp/gev-npm-ci.log | head -1 || echo 'npm ci clean'))"
  if grep -q 'install-scripts' /tmp/gev-npm-ci.log; then
    note 'npm >= 11.19 deferred the esbuild/puppeteer postinstalls — harmless for dev, build and tests'
    note 'only the qa:* scripts need them: npm install-scripts approve puppeteer && npm install'
  fi
else
  fail 'npm ci failed — see /tmp/gev-npm-ci.log'
  tail -5 /tmp/gev-npm-ci.log | sed 's/^/       /'
fi

# --- configuration ---------------------------------------------------------

step 'Configuration'
if [ -f .env ]; then
  ok '.env already present — left untouched'
  perms="$(stat -c '%a' .env 2>/dev/null || stat -f '%Lp' .env 2>/dev/null)"
  [ "${perms:-600}" = 600 ] || note ".env is mode $perms; it holds API keys — chmod 600 .env"
elif [ "$CHECK_ONLY" -eq 1 ]; then
  skip 'no .env (check mode: not creating one)'
elif [ -f .env.example ]; then
  # Create owner-only *before* any content lands in it.
  ( umask 077 && cp .env.example .env ) && chmod 600 .env \
    && ok '.env created from .env.example, mode 600' \
    || fail 'could not create .env'
else
  skip 'no .env.example in this checkout'
fi
note 'keys are optional — GEV runs keyless. Add them in-app via the POWER UP chip.'

if [ "$CHECK_ONLY" -eq 0 ] && [ -d node_modules ]; then
  step 'Doctor'
  if npm run doctor 2>&1 | tee /tmp/gev-doctor.log | sed 's/^/       /'; then
    ok 'setup doctor ran'
  else
    fail 'setup doctor reported a problem — see /tmp/gev-doctor.log'
  fi
fi

# --- optional verification -------------------------------------------------

if [ "$DO_VERIFY" -eq 1 ] && [ "$CHECK_ONLY" -eq 0 ]; then
  step 'Verification'
  if npm run build >/tmp/gev-build.log 2>&1; then
    ok "production build ($(grep -oE 'built in [0-9.]+s' /tmp/gev-build.log | tail -1 || echo 'done'))"
  else
    fail 'build failed — see /tmp/gev-build.log'
  fi
  if npm test >/tmp/gev-test.log 2>&1; then
    ok "unit suite ($(grep -oE 'pass [0-9]+' /tmp/gev-test.log | tail -1 || echo 'passed') passing)"
  else
    fail 'unit suite failed — see /tmp/gev-test.log'
  fi
fi

# --- start -----------------------------------------------------------------

if [ "$DO_START" -eq 1 ]; then
  step 'Start'
  if ! [ -d node_modules ]; then
    skip 'dependencies missing — not starting'
  else
    LOG=/tmp/gev-dev.log
    # HOST stays unset: the dev server brokers your API keys, so it should
    # listen on localhost only unless you have deliberately opted into LAN.
    PORT="$PORT" nohup npm run dev >"$LOG" 2>&1 &
    DEV_PID=$!
    for _ in $(seq 1 60); do
      kill -0 "$DEV_PID" 2>/dev/null || break
      if curl -fsS --noproxy localhost -o /dev/null "http://localhost:${PORT}/" 2>/dev/null; then
        ok "running at http://localhost:${PORT}/ (pid $DEV_PID)"
        note "log: $LOG   ·   stop: kill $DEV_PID"
        break
      fi
      sleep 1
    done
    if ! curl -fsS --noproxy localhost -o /dev/null "http://localhost:${PORT}/" 2>/dev/null; then
      if kill -0 "$DEV_PID" 2>/dev/null; then
        skip "server started (pid $DEV_PID) but did not answer on ${PORT} within 60s — check $LOG"
      else
        fail "dev server exited — see $LOG"
        tail -5 "$LOG" | sed 's/^/       /'
      fi
    fi
  fi
fi

# --- summary ---------------------------------------------------------------

step 'Summary'
for line in "${SUMMARY[@]}"; do
  printf '  %-5s %s\n' "${line%%|*}" "${line#*|}"
done
if [ "$FAILURES" -eq 0 ]; then
  printf '\nAll steps that could run, ran.\n'
else
  printf '\n%d step(s) failed; the rest completed.\n' "$FAILURES"
fi
exit "$FAILURES"
