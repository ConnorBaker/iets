# fcheck.awk [-v skip=REGEX] fchg.all TRACES: for each job, each step that changed something its fine trace observed.
# fchg.all holds "step\tchange" lines; TRACES holds "J\tjob" and then the job's trace.
# skip drops the whole reads (A) whose Nix call or stack matches it: what a fix at those sites would give.
BEGIN { FS = OFS = "\t" }
function init_ascii(    i, c) {
  for (i = 32; i < 127; i++) {
    c = sprintf("%c", i)
    if (c ~ /^[a-zA-Z0-9._+\/-]$/) ascii[sprintf("%02x", i)] = c
  }
}
FILENAME == ARGV[1] {
  s = $1
  step[s] = 1
  if ($2 == "V" && $3 == "2") version[s] = 1
  else if ($2 == "CR") cr[$3 "\t" $4] = cr[$3 "\t" $4] " " s
  else if ($2 == "CF") cf[$3] = cf[$3] " " s
  else if ($2 == "CL") { cl[$3 "\t" $4 "\t" $5] = cl[$3 "\t" $4 "\t" $5] ";" s ":" $6; n = split($6, x, " "); for (i = 1; i <= n; i++) cn[x[i]] = 1 }
  else if ($2 == "CD") { cd[$3] = cd[$3] ";" s ":" $4; cn[$4] = 1 }
  else if ($2 == "CM") cm[$3] = cm[$3] " " s
  else if ($2 == "CT") ct[$3] = ct[$3] " " s
  else if ($2 == "CS") cs[$3] = cs[$3] " " s
  else if ($2 == "CX") cx[$3] = cx[$3] ";" s ":" $4
  next
}
function mark(ss, why,    n, x, i) { n = split(ss, x, " "); for (i = 1; i <= n; i++) if (!(x[i] in inv)) inv[x[i]] = why }
function entries(e, name, why,    n, x, i, y) {
  n = split(e, x, ";")
  for (i = 2; i <= n; i++) {
    split(x[i], y, ":")
    if (name == "" || y[2] == "*" || index(" " y[2] " ", " " name " ")) mark(y[1], why)
  }
}
function flush(   s) {
  if (job != "") {
    if (!complete) {
      print "regenerate the fine job trace for " job " with the current trace format" > "/dev/stderr"
      failed = 1
    } else for (s in inv) print job, s, inv[s]
  }
  delete inv; delete pc; complete = started = 0
}
$1 == "J" { flush(); job = $2; next }
$0 == "B" && !started { started = 1; next }
$0 == "V\tjob-trace-cumulative-v1" && started && !complete { complete = 1; next }
!started || complete || $0 == "B" || /^V/ { print "invalid fine job trace for " job > "/dev/stderr"; failed = 1; next }
$1 == "R" && $2 == "?" { for (s in step) inv[s] = "untracked " $3; next }
$1 == "E" || $1 == "Q" || $1 == "I" || $1 == "TreeMtime" { for (s in step) inv[s] = "external read " $1; next }
$1 == "AttrOrder" { for (s in step) inv[s] = "attrset comparison order"; next }
$1 == "R" { k = $2 "\t" $3; if (k in cr) mark(cr[k], "R " $2 " " $3); if ($2 in cf) mark(cf[$2], "R " $2); next }
$1 == "P" {
  if ($3 == "lit") pc[$2] = cl[$4 "\t" $5 "\t" $6]
  else if ($3 == "dir") pc[$2] = cd[$4]
  else if ($3 == "u") pc[$2] = pc[$4] pc[$5]
  else if ($3 == "x") pc[$2] = pc[$4]
  else print job, "?", "unknown provenance " $0
  next
}
$1 == "K" { if (pc[$2] != "" && ($3 in cn || index(pc[$2], ":*"))) entries(pc[$2], $3, "K " $3); next }
$1 == "A" { if (pc[$2] != "" && !(skip != "" && ($4 " " $5) ~ skip)) entries(pc[$2], "", "A " $3 " " $4); next }
$1 == "X" { if ($2 in cx) { n = split(cx[$2], x, ";"); for (i = 2; i <= n; i++) { split(x[i], y, ":"); if (y[2] + 0 <= $3 + 0) mark(y[1], "X " $2) } } next }
$1 == "F" { if ($2 in cm) mark(cm[$2], $1 " " $2); next }
$1 == "C" {
  path = $2
  if (substr(path, 1, 1) == "~") {
    if (!("2f" in ascii)) init_ascii()
    # The plain prefix ends at the first byte that requires encoding.
    plain = 1; prefix = ""
    for (i = 2; i <= length(path); i += 2) {
      byte = substr(path, i, 2)
      if (byte !~ /^[0-9a-f][0-9a-f]$/) { for (s in step) inv[s] = "malformed path"; next }
      if (byte == "2f") {
        key = i == 2 ? "/" : (plain ? prefix : substr(path, 1, i - 1))
        if (key in cm) mark(cm[key], "C " key)
      }
      if (plain && byte in ascii) prefix = prefix ascii[byte]
      else plain = 0
    }
    if (path in cm) mark(cm[path], "C " path)
    next
  }
  while (path != "") {
    if (path in cm) mark(cm[path], "C " path)
    if (!sub(/\/[^/]*$/, "", path)) break
  }
  next
}
$1 == "T" { if ($2 in ct) mark(ct[$2], "T " $2); next }
$1 == "S" { if ($2 in cs) mark(cs[$2], "S " $2); next }
$1 == "D" { if ($2 in cd) entries(cd[$2], "", "D " $2); next }
END {
  flush()
  for (s in step) if (!(s in version)) { print "regenerate the changes" > "/dev/stderr"; failed = 1; break }
  exit failed
}
