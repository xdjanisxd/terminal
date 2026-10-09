# Session-local wrapper: load user initialization before installing prompt hooks.
if [[ -n ${__TERMINAL_BASHRC+x} ]]; then
    PROMPT_COMMAND=${__TERMINAL_PRIOR_PROMPT_COMMAND-}
    unset __TERMINAL_PRIOR_PROMPT_COMMAND
fi
if [[ $__TERMINAL_NO_RC != 1 ]]; then
    if [[ -n ${__TERMINAL_RCFILE+x} ]]; then
        [[ -r $__TERMINAL_RCFILE ]] && builtin source "$__TERMINAL_RCFILE"
    else
        [[ -r ~/.bashrc ]] && builtin source ~/.bashrc
    fi
fi
unset __TERMINAL_NO_RC __TERMINAL_RCFILE

__terminal_report_prompt() {
    local __terminal_status=$? LC_ALL=C __terminal_path= __terminal_char __terminal_hex __terminal_i
    # Apply a restored root once, after all user initialization/prompt hooks.
    if [[ -n ${__TERMINAL_ROOT+x} ]]; then
        builtin cd -- "$__TERMINAL_ROOT" || :
        unset __TERMINAL_ROOT
    fi
    for ((__terminal_i=0; __terminal_i<${#PWD}; __terminal_i++)); do
        __terminal_char=${PWD:__terminal_i:1}
        case $__terminal_char in
            [a-zA-Z0-9/._~-]) __terminal_path+=$__terminal_char ;;
            *) builtin printf -v __terminal_hex '%%%02X' "'$__terminal_char"; __terminal_path+=$__terminal_hex ;;
        esac
    done
    builtin printf '\033]7;file://%s\007\033]133;A\007' "$__terminal_path"
    return "$__terminal_status"
}
# Append to scalar or array PROMPT_COMMAND, retaining existing hooks.
if [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == 'declare -a '* ]]; then
    PROMPT_COMMAND+=(__terminal_report_prompt)
else
    PROMPT_COMMAND="${PROMPT_COMMAND:+$PROMPT_COMMAND; }__terminal_report_prompt"
fi

# --norc keeps both system and user rc disabled; its bootstrap runs as the
# first PROMPT_COMMAND rather than through --rcfile.
if [[ -n ${__TERMINAL_BASHRC+x} ]]; then
    unset __TERMINAL_BASHRC
    builtin eval "$PROMPT_COMMAND"
fi
