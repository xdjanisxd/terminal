__terminal_report_prompt() {
    local __terminal_status=$?
    emulate -L zsh
    if [[ -n ${__TERMINAL_INTEGRATION_DIR-} ]]; then
        if (( ${+__TERMINAL_ZDOTDIR} )); then
            export ZDOTDIR=$__TERMINAL_ZDOTDIR
        else
            unset ZDOTDIR
        fi
        if (( ${+__TERMINAL_ROOT} )); then
            builtin cd -- "$__TERMINAL_ROOT" || :
        fi
        unset __TERMINAL_ROOT __TERMINAL_ZDOTDIR __TERMINAL_INTEGRATION_DIR __TERMINAL_NO_RC
        unfunction __terminal_source_user
    fi
    local LC_ALL=C __terminal_path= __terminal_char __terminal_hex __terminal_i
    for ((__terminal_i=1; __terminal_i<=${#PWD}; __terminal_i++)); do
        __terminal_char=$PWD[__terminal_i]
        case $__terminal_char in
            [a-zA-Z0-9/._~-]) __terminal_path+=$__terminal_char ;;
            *) builtin printf -v __terminal_hex '%%%02X' "'$__terminal_char"; __terminal_path+=$__terminal_hex ;;
        esac
    done
    builtin printf '\033]7;file://%s\007\033]133;A\007' "$__terminal_path"
    return $__terminal_status
}
# No autoload dependency; preserve the user's precmd and precmd_functions.
typeset -ga precmd_functions
precmd_functions+=(__terminal_report_prompt)
