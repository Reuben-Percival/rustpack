# bash completion for rustpack
_rustpack() {
    local cur prev words cword
    _init_completion || return

    local ops="-S -Q -R -U --why doctor history"
    local global_opts="--help -h --test --dry-run --noconfirm --needed --nodeps --noscriptlet --overwrite --asdeps --asexplicit --root --dbpath --cachedir --strict --insecure-skip-signatures --compact --verbose --json --aur --paru --"
    local s_opts="-Sy -Su -Syu -Ss -Si -Sc -Scc -Sd -Sdd"
    local q_opts="-Qi -Qs -Ql -Qm -Qo -Qe -Qr"
    local r_opts="-Rs -Rn -Rd -Rdd"
    local u_opts="-Ud -Udd"

    prev="${COMP_WORDS[COMP_CWORD-1]}"

    case "$prev" in
        --root|--dbpath|--cachedir)
            COMPREPLY=( $(compgen -d -- "$cur") )
            return
            ;;
        --overwrite)
            COMPREPLY=()
            return
            ;;
        history)
            COMPREPLY=( $(compgen -W "show" -- "$cur") )
            return
            ;;
        show)
            return
            ;;
    esac

    if [[ ${#COMP_WORDS[@]} -eq 2 ]]; then
        COMPREPLY=( $(compgen -W "$ops $global_opts" -- "$cur") )
        return
    fi

    if [[ " ${COMP_WORDS[*]} " == *" history "* ]]; then
        COMPREPLY=( $(compgen -W "show" -- "$cur") )
        return
    fi

    if [[ " ${COMP_WORDS[*]} " == *" -S "* || " ${COMP_WORDS[*]} " == *" -Sy "* || " ${COMP_WORDS[*]} " == *" -Su "* || " ${COMP_WORDS[*]} " == *" -Syu "* ]]; then
        COMPREPLY=( $(compgen -W "$s_opts $global_opts" -- "$cur") )
        return
    fi

    if [[ " ${COMP_WORDS[*]} " == *" -Q "* ]]; then
        COMPREPLY=( $(compgen -W "$q_opts $global_opts" -- "$cur") )
        return
    fi

    if [[ " ${COMP_WORDS[*]} " == *" -R "* ]]; then
        COMPREPLY=( $(compgen -W "$r_opts $global_opts" -- "$cur") )
        return
    fi

    if [[ " ${COMP_WORDS[*]} " == *" -U "* ]]; then
        COMPREPLY=( $(compgen -W "$u_opts $global_opts" -- "$cur") )
        return
    fi

    COMPREPLY=( $(compgen -W "$ops $global_opts $s_opts $q_opts $r_opts $u_opts" -- "$cur") )
}

complete -F _rustpack rustpack
