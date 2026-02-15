# fish completion for rustpack

set -l global_opts --test --dry-run --noconfirm --needed --nodeps --noscriptlet --asdeps --asexplicit --strict --insecure-skip-signatures --compact --verbose --json --aur --paru

complete -c rustpack -f -n "__fish_use_subcommand" -a "-S -Q -R -U --why doctor history"
complete -c rustpack -f -l help -s h -d "Show help"

complete -c rustpack -f -n "__fish_seen_subcommand_from -S" -a "-Sy -Su -Syu -Ss -Si -Sc -Scc -Sd -Sdd"
complete -c rustpack -f -n "__fish_seen_subcommand_from -Q" -a "-Qi -Qs -Ql -Qm -Qo -Qe -Qr"
complete -c rustpack -f -n "__fish_seen_subcommand_from -R" -a "-Rs -Rn -Rd -Rdd"
complete -c rustpack -f -n "__fish_seen_subcommand_from -U" -a "-Ud -Udd"

for opt in $global_opts
    complete -c rustpack -f -a $opt
end

complete -c rustpack -f -l root -r -d "Use alternate root"
complete -c rustpack -f -l dbpath -r -d "Use alternate db path"
complete -c rustpack -f -l cachedir -r -d "Use alternate cache dir"
complete -c rustpack -f -l overwrite -r -d "Overwrite conflicting files"

complete -c rustpack -f -n "__fish_seen_subcommand_from history" -a "show"
