#!/usr/bin/env bash
# fchg/<n>.txt: what step n changed, in the terms of the fine trace (fcheck.awk reads it)
set -eu -o pipefail
export LC_ALL=C
cd ~/incr/nixpkgs
P=$HOME/incr/base
I=$HOME/iets-incr/target/release/iets
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
mkdir -p ../fchg
cs=($(git log --first-parent --format=%h --reverse 9dbd131e6c65~1..4e63084a455b))
pairs() { for ((i = 1; i < ${#cs[@]}; i++)); do echo "$i ${cs[i-1]} ${cs[i]}"; done; echo "w100 ${cs[0]} ${cs[100]}"; for ((i = 10; i <= 100; i += 10)); do echo "w10-$i ${cs[i-10]} ${cs[i]}"; done; }
pairs | while read n a b; do
  out=../fchg/$n.txt
  # Older outputs skipped imported files whose name did not end in .nix.
  # Only reuse a file published for this pair by the corrected generator.
  receipt=../fchg/.all-imports-$a-$b
  if [[ -s $out && $out -ef $receipt ]]; then continue; fi
  staged=$(mktemp ../fchg/.changes.XXXXXXXX)
  trap 'rm -f -- "$staged"' EXIT
  git diff --no-renames --name-status $a $b | while IFS=$'\t' read st p; do
    printf "CM\t%s/%s\n" $P "$p"
    q=$p
    while :; do printf "CT\t%s/%s\n" $P "$q"; [[ $q == */* ]] || break; q=${q%/*}; done
    printf "CT\t%s\n" $P
    case $st in
    A | D)
      [ $st = A ] && other=$a || other=$b
      pre=""
      IFS=/ read -ra parts <<< "$p"
      for ((j = 0; j < ${#parts[@]}; j++)); do
        cand=${pre:+$pre/}${parts[j]}
        if ! git cat-file -e $other:"$cand" 2>/dev/null; then
          printf "CD\t%s\t%s\n" "$P${pre:+/$pre}" "${parts[j]}"
          x=$cand
          printf "CS\t%s/%s\n" $P "$x"
          for ((k = j + 1; k < ${#parts[@]}; k++)); do x=$x/${parts[k]}; printf "CS\t%s/%s\n" $P "$x"; done
          break
        fi
        pre=$cand
      done
      ;;
    T)
      d=$(dirname "$p"); [ "$d" = . ] && d="" || d=/$d
      printf "CD\t%s\t%s\n" "$P$d" "$(basename "$p")"
      printf "CS\t%s/%s\n" $P "$p"
      ;;
    esac
    # Nix allows an import to name any file. Its reads may be recorded as
    # code regions and literal attributes even without a raw file read.
    if [[ $st != A ]]; then
      git show $a:"$p" > $T/a.nix
      "$I" fine-regions "$T/a.nix=$P/$p" > $T/a.tab 2>/dev/null
      if [ $st = D ]; then echo E > $T/b.tab; else git show $b:"$p" > $T/b.nix; "$I" fine-regions "$T/b.nix=$P/$p" > $T/b.tab 2>/dev/null; fi
      grep -q . $T/a.tab || echo E > $T/a.tab
      grep -q . $T/b.tab || echo E > $T/b.tab
      gawk -v f="$P/$p" -f ../fcmp.awk $T/a.tab $T/b.tab
      if [[ $st == D ]]; then
        printf 'CX\t%s/%s\t0\n' "$P" "$p"
      elif ! cmp -s "$T/a.nix" "$T/b.nix"; then
        # On a prefix-only edit cmp reports EOF on stderr without an offset.
        d=$(cd "$T"; { cmp a.nix b.nix 2>/dev/null || :; } | gawk '$5 ~ /^[0-9]+,?$/ {print $5 + 0}')
        printf 'CX\t%s/%s\t%d\n' "$P" "$p" "$((${d:-1} - 1))"
      fi
    fi
  done | sort -u > "$staged"
  ln -f -- "$staged" "$receipt"
  mv -f -- "$staged" "$out"
done
